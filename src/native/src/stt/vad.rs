//! Energy voice-activity detection over fixed 30 ms frames.
//!
//! No ML, no deps: RMS threshold + minimum-speech gate (rejects clicks and
//! coughs) + silence hangover (lets the user pause mid-sentence without
//! cutting them off). Tuned for close-talk laptop mics; every knob is env
//! overridable (see [`VadConfig::from_env`]).
//!
//! The detector is deliberately dumb — whisper judges content, this only
//! decides *when* to record. Short commands on `tiny` want generous hangover
//! over aggressive cutoff: a late end wastes a second of CPU, an early end
//! loses the last word.

/// 30 ms frames at 16 kHz.
pub const FRAME_SAMPLES: usize = 480;

#[derive(Debug, Clone)]
pub struct VadConfig {
    /// RMS at or above this counts as speech.
    pub threshold: f32,
    /// Silent frames to wait before ending an utterance.
    pub silence_frames: usize,
    /// Speech frames required before an utterance officially starts
    /// (pre-roll covers the gap so nothing is lost).
    pub min_speech_frames: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: 0.02,
            silence_frames: 40, // 1200 ms
            min_speech_frames: 13, // ~400 ms
        }
    }
}

impl VadConfig {
    pub fn from_env() -> Self {
        fn env_f(key: &str) -> Option<f32> {
            std::env::var(key)
                .ok()
                .and_then(|v| v.trim().parse().ok())
        }
        fn env_ms(key: &str) -> Option<usize> {
            std::env::var(key)
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(|ms| (ms / 30).max(1) as usize)
        }
        let d = Self::default();
        Self {
            threshold: env_f("VOICE_THRESHOLD")
                .filter(|v| *v > 0.0)
                .unwrap_or(d.threshold),
            silence_frames: env_ms("VOICE_SILENCE_MS").unwrap_or(d.silence_frames),
            min_speech_frames: env_ms("VOICE_MIN_SPEECH_MS")
                .unwrap_or(d.min_speech_frames),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadTransition {
    Silence,
    SpeechOngoing,
    /// min_speech gate passed: recording (with pre-roll) starts now.
    SpeechStart,
    /// hangover elapsed: the utterance is complete, transcribe it.
    SpeechEnd,
}

#[derive(Debug)]
enum State {
    Idle,
    MaybeSpeech(usize),
    Speech(usize),
}

#[derive(Debug)]
pub struct Vad {
    cfg: VadConfig,
    state: State,
}

impl Vad {
    pub fn new(cfg: VadConfig) -> Self {
        Self {
            cfg,
            state: State::Idle,
        }
    }

    /// Introspection for diagnostics (tuning without a debugger attached).
    pub fn threshold(&self) -> f32 {
        self.cfg.threshold
    }

    pub fn silence_ms(&self) -> u64 {
        self.cfg.silence_frames as u64 * 30
    }

    pub fn min_speech_ms(&self) -> u64 {
        self.cfg.min_speech_frames as u64 * 30
    }

    pub fn feed(&mut self, rms: f32) -> VadTransition {
        let speech = rms >= self.cfg.threshold;
        match (&self.state, speech) {
            (State::Idle, false) => VadTransition::Silence,
            (State::Idle, true) => {
                self.state = State::MaybeSpeech(1);
                VadTransition::Silence
            }
            (State::MaybeSpeech(n), true) => {
                let n = *n + 1;
                if n >= self.cfg.min_speech_frames {
                    self.state = State::Speech(0);
                    VadTransition::SpeechStart
                } else {
                    self.state = State::MaybeSpeech(n);
                    VadTransition::Silence
                }
            }
            (State::MaybeSpeech(_), false) => {
                self.state = State::Idle;
                VadTransition::Silence
            }
            (State::Speech(_), true) => {
                self.state = State::Speech(0);
                VadTransition::SpeechOngoing
            }
            (State::Speech(sil), false) => {
                let sil = *sil + 1;
                if sil >= self.cfg.silence_frames {
                    self.state = State::Idle;
                    VadTransition::SpeechEnd
                } else {
                    self.state = State::Speech(sil);
                    VadTransition::SpeechOngoing
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VadConfig {
        VadConfig {
            threshold: 0.02,
            silence_frames: 4,
            min_speech_frames: 3,
        }
    }

    #[test]
    fn silence_stays_silent() {
        let mut v = Vad::new(cfg());
        for _ in 0..20 {
            assert_eq!(v.feed(0.001), VadTransition::Silence);
        }
    }

    #[test]
    fn short_blip_below_min_speech_is_ignored() {
        let mut v = Vad::new(cfg());
        assert_eq!(v.feed(0.1), VadTransition::Silence);
        assert_eq!(v.feed(0.1), VadTransition::Silence);
        // Back to quiet before the gate: no start, still idle.
        assert_eq!(v.feed(0.001), VadTransition::Silence);
        assert_eq!(v.feed(0.1), VadTransition::Silence);
        assert_eq!(v.feed(0.001), VadTransition::Silence);
    }

    #[test]
    fn sustained_speech_starts_then_hangover_ends() {
        let mut v = Vad::new(cfg());
        assert_eq!(v.feed(0.1), VadTransition::Silence);
        assert_eq!(v.feed(0.1), VadTransition::Silence);
        assert_eq!(v.feed(0.1), VadTransition::SpeechStart);
        assert_eq!(v.feed(0.1), VadTransition::SpeechOngoing);
        // Hangover: still ongoing through the grace frames…
        assert_eq!(v.feed(0.001), VadTransition::SpeechOngoing);
        assert_eq!(v.feed(0.001), VadTransition::SpeechOngoing);
        assert_eq!(v.feed(0.001), VadTransition::SpeechOngoing);
        // …then the utterance ends.
        assert_eq!(v.feed(0.001), VadTransition::SpeechEnd);
        assert_eq!(v.feed(0.001), VadTransition::Silence);
    }

    #[test]
    fn speech_resumes_reset_the_hangover() {
        let mut v = Vad::new(cfg());
        for _ in 0..3 {
            v.feed(0.1);
        }
        v.feed(0.001);
        v.feed(0.001);
        // Speech again before the hangover lapses: no end, no restart.
        assert_eq!(v.feed(0.1), VadTransition::SpeechOngoing);
        for _ in 0..3 {
            assert_eq!(v.feed(0.001), VadTransition::SpeechOngoing);
        }
        assert_eq!(v.feed(0.001), VadTransition::SpeechEnd);
    }

    #[test]
    fn threshold_boundary() {
        let mut v = Vad::new(cfg());
        // Exactly at threshold counts as speech.
        assert_eq!(v.feed(0.02), VadTransition::Silence);
        assert_eq!(v.feed(0.02), VadTransition::Silence);
        assert_eq!(v.feed(0.02), VadTransition::SpeechStart);
    }
}
