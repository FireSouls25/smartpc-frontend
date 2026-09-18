//! Voice sessions: one mic, one listener thread, one event queue.
//!
//! Modes:
//!
//! - `manual`: press to start, VAD captures the first utterance, a silence
//!   hangover finalizes it, whisper transcribes, the session ends. Nobody
//!   presses anything to *finish*.
//! - `wake`: armed until the wake word ("hey" default, configurable) is
//!   heard in a transcribed onset utterance, then the *next* utterance is
//!   the command; re-arms until stopped.
//!
//! The frontend long-polls `events` (cursor-based, 25 s server hold) and
//! feeds final transcripts straight into the agent. Everything is local:
//! mic → VAD → tiny → text, no network after the first model download.
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use tokio::sync::Notify;

use super::audio::{self, TARGET_RATE};
use super::vad::{FRAME_SAMPLES, Vad, VadConfig, VadTransition};
use super::engine::Engine;
use super::model;
use super::wake::contains_wake_word;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenMode {
    Manual,
    Wake,
}

impl ListenMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "manual" => Some(Self::Manual),
            "wake" => Some(Self::Wake),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Wake => "wake",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ListenOpts {
    pub mode: ListenMode,
    pub wake_word: String,
    pub lang: String,
    pub model: String,
}

#[derive(Debug)]
pub enum StartError {
    AlreadyListening,
    NoMicrophone,
    BadMode,
    BadWakeWord,
    BadLang,
    ModelFailed(String),
    CaptureFailed(String),
}

impl StartError {
    pub fn http_parts(&self) -> (u16, &'static str, String) {
        match self {
            Self::AlreadyListening => (
                409,
                "already_listening",
                "voice session already active (stop it first)".to_string(),
            ),
            Self::NoMicrophone => (
                503,
                "no_microphone",
                "no microphone found on this machine".to_string(),
            ),
            Self::BadMode => (
                400,
                "invalid_mode",
                "mode must be manual|wake".to_string(),
            ),
            Self::BadWakeWord => (
                400,
                "invalid_wake_word",
                "wake word must be 1-32 characters".to_string(),
            ),
            Self::BadLang => (
                400,
                "invalid_lang",
                "lang must be a 2-letter code (es, en, …)".to_string(),
            ),
            Self::ModelFailed(e) => (502, "model_failed", e.clone()),
            Self::CaptureFailed(e) => (500, "capture_failed", e.clone()),
        }
    }
}

/// Parse + validate the wire options before touching any device.
pub fn parse_opts(
    mode: Option<&str>,
    wake_word: Option<&str>,
    lang: Option<&str>,
    model: Option<&str>,
) -> Result<ListenOpts, StartError> {
    let mode = ListenMode::parse(mode.unwrap_or("manual")).ok_or(StartError::BadMode)?;
    let wake_word = wake_word.unwrap_or("hey").trim().to_string();
    if wake_word.is_empty() || wake_word.chars().count() > 32 {
        return Err(StartError::BadWakeWord);
    }
    let lang = lang.unwrap_or("es").trim().to_lowercase();
    if lang.len() != 2 || !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(StartError::BadLang);
    }
    Ok(ListenOpts {
        mode,
        wake_word,
        lang,
        model: model::model_name_or_default(model),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceEvent {
    Capturing { active: bool },
    Wake { word: String },
    Transcript { text: String },
    Error { code: String, message: String },
    End {},
}

#[derive(Debug, Clone)]
struct StoredEvent {
    seq: u64,
    event: VoiceEvent,
}

struct SessionHandle {
    stop: Arc<AtomicBool>,
    capturing: Arc<AtomicBool>,
    mode: ListenMode,
    wake_word: String,
}

struct Inner {
    session: Option<SessionHandle>,
    events: VecDeque<StoredEvent>,
    next_seq: u64,
}

#[derive(Clone)]
pub struct VoiceService {
    inner: Arc<Mutex<Inner>>,
    notify: Arc<Notify>,
    models_dir: std::path::PathBuf,
}

impl VoiceService {
    pub fn new(models_dir: std::path::PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                session: None,
                events: VecDeque::with_capacity(64),
                next_seq: 1,
            })),
            notify: Arc::new(Notify::new()),
            models_dir,
        }
    }

    fn push_event(&self, event: VoiceEvent) {
        if let Ok(mut inner) = self.inner.lock() {
            let seq = inner.next_seq;
            inner.next_seq += 1;
            inner.events.push_back(StoredEvent { seq, event });
            while inner.events.len() > 200 {
                inner.events.pop_front();
            }
        }
        self.notify.notify_one();
    }

    /// Snapshot for `GET /v1/voice/status`. Never blocks on audio.
    pub fn status(&self) -> serde_json::Value {
        let (listening, capturing, mode, wake_word) = match self.inner.lock() {
            Ok(inner) => match &inner.session {
                Some(s) => (
                    true,
                    s.capturing.load(Ordering::Relaxed),
                    Some(s.mode.as_str().to_string()),
                    Some(s.wake_word.clone()),
                ),
                None => (false, false, None, None),
            },
            Err(_) => (false, false, None, None),
        };
        let model = model::model_name_or_default(None);
        serde_json::json!({
            "listening": listening,
            "capturing": capturing,
            "mode": mode,
            "model": model,
            "wake_word": wake_word,
            "mic": audio::microphone_present(),
            "model_ready": model::model_ready(&self.models_dir, &model),
        })
    }

    /// Start listening. Blocks (call from `spawn_blocking`): model download
    /// first, then mic open, then the listener thread owns the rest.
    pub fn start(&self, opts: ListenOpts) -> Result<(), StartError> {
        let stop = Arc::new(AtomicBool::new(false));
        let capturing = Arc::new(AtomicBool::new(false));
        {
            let mut inner = self.inner.lock().map_err(|_| {
                StartError::CaptureFailed("voice state poisoned".to_string())
            })?;
            if inner.session.is_some() {
                return Err(StartError::AlreadyListening);
            }
            // Claimed before any blocking work: a second start() fails fast
            // instead of stacking downloads/threads.
            inner.session = Some(SessionHandle {
                stop: stop.clone(),
                capturing: capturing.clone(),
                mode: opts.mode,
                wake_word: opts.wake_word.clone(),
            });
        }
        if !audio::microphone_present() {
            self.clear_session();
            return Err(StartError::NoMicrophone);
        }
        let model_path = match tokio_block_on(model::ensure_downloaded(
            &self.models_dir,
            &opts.model,
        )) {
            Ok(p) => p,
            Err(e) => {
                self.clear_session();
                return Err(StartError::ModelFailed(e));
            }
        };
        let engine = match Engine::load(&model_path) {
            Ok(e) => e,
            Err(e) => {
                self.clear_session();
                return Err(StartError::ModelFailed(e));
            }
        };
        let service = self.clone();
        let thread_stop = stop.clone();
        let thread_capturing = capturing.clone();
        if std::thread::Builder::new()
            .name("smartpc-voice".to_string())
            .spawn(move || {
                run_listener(service, thread_stop, thread_capturing, opts, engine);
            })
            .is_err()
        {
            self.clear_session();
            return Err(StartError::CaptureFailed("voice thread failed".to_string()));
        }
        // Detached: the thread clears the session itself on exit
        // (auto-stop, error, or stop flag).
        Ok(())
    }

    fn clear_session(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.session = None;
        }
    }

    /// Ask the listener to stop. Idempotent; the thread pushes `End`.
    pub fn stop(&self) {
        let stop = match self.inner.lock() {
            Ok(inner) => inner.session.as_ref().map(|s| s.stop.clone()),
            Err(_) => None,
        };
        if let Some(flag) = stop {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Long-poll: events after `cursor`, waiting up to ~25 s. Returns
    /// `(events, next_cursor)`; empty events + same cursor means "no news".
    pub async fn poll(&self, cursor: u64) -> (Vec<serde_json::Value>, u64) {
        let deadline = std::time::Instant::now() + Duration::from_secs(25);
        loop {
            let drained = match self.inner.lock() {
                Ok(inner) => {
                    let mut out = Vec::new();
                    let mut next = cursor;
                    for stored in inner.events.iter() {
                        if stored.seq > cursor {
                            let mut v = serde_json::to_value(&stored.event)
                                .unwrap_or(serde_json::Value::Null);
                            v["seq"] = serde_json::Value::from(stored.seq);
                            out.push(v);
                            next = stored.seq;
                        }
                    }
                    Some((out, next.max(cursor)))
                }
                Err(_) => None,
            };
            match drained {
                Some((out, next)) if !out.is_empty() => return (out, next),
                _ => {}
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return (Vec::new(), cursor);
            }
            tokio::select! {
                _ = self.notify.notified() => {},
                _ = tokio::time::sleep(remaining) => {},
            }
        }
    }
}

/// Minimal block_on for the model download inside `start` (which itself runs
/// in `spawn_blocking`, so no runtime exists here).
fn tokio_block_on<F>(fut: F) -> F::Output
where
    F: std::future::Future,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("voice helper runtime")
        .block_on(fut)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    AwaitingWake,
    AwaitingCommand,
}

fn run_listener(
    service: VoiceService,
    stop: Arc<AtomicBool>,
    capturing: Arc<AtomicBool>,
    opts: ListenOpts,
    engine: Engine,
) {
    let liga = Listener::new(service.clone(), stop, capturing, opts, engine);
    liga.run();
    // Session record cleared here (auto-stop, error, or stop flag) so a new
    // listen() can start; the End event is already queued.
    if let Ok(mut inner) = service.inner.lock() {
        inner.session = None;
    }
    service.notify.notify_one();
}

struct Listener {
    service: VoiceService,
    stop: Arc<AtomicBool>,
    capturing_flag: Arc<AtomicBool>,
    opts: ListenOpts,
    engine: Engine,
    vad: Vad,
    preroll: VecDeque<f32>,
    utter: Vec<f32>,
    capturing: bool,
    arm: Arm,
    max_utter_samples: usize,
}

impl Listener {
    fn new(
        service: VoiceService,
        stop: Arc<AtomicBool>,
        capturing_flag: Arc<AtomicBool>,
        opts: ListenOpts,
        engine: Engine,
    ) -> Self {
        let max_s: u64 = std::env::var("VOICE_MAX_UTTERANCE_S")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(30);
        Self {
            service,
            stop,
            capturing_flag,
            opts,
            engine,
            vad: Vad::new(VadConfig::from_env()),
            preroll: VecDeque::with_capacity(FRAME_SAMPLES * 10),
            utter: Vec::with_capacity(TARGET_RATE as usize * 8),
            capturing: false,
            arm: Arm::AwaitingWake,
            max_utter_samples: (max_s.max(5) as usize) * TARGET_RATE as usize,
        }
    }

    fn emit(&self, event: VoiceEvent) {
        self.service.push_event(event);
    }

    fn set_capturing(&mut self, on: bool) {
        self.capturing = on;
        self.capturing_flag.store(on, Ordering::Relaxed);
        self.emit(VoiceEvent::Capturing { active: on });
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn run(mut self) {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<f32>, String>>(128);
        let _stream = match audio::open_capture(tx.clone()) {
            Ok(s) => s,
            Err(e) => {
                let (code, message) = match &e {
                    audio::CaptureError::NoMicrophone => (
                        "no_microphone",
                        "no microphone found on this machine",
                    ),
                    audio::CaptureError::Unsupported(d) => ("unsupported_audio", d.as_str()),
                    audio::CaptureError::Stream(d) => ("capture_failed", d.as_str()),
                };
                self.emit(VoiceEvent::Error {
                    code: code.to_string(),
                    message: message.to_string(),
                });
                self.emit(VoiceEvent::End {});
                return;
            }
        };
        crate::diagnostics::push(format!(
            "voice: listening (mode={}, lang={})",
            self.opts.mode.as_str(),
            self.opts.lang
        ));
        while !self.stopped() {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(frame)) => self.on_frame(&frame),
                Ok(Err(e)) => {
                    self.emit(VoiceEvent::Error {
                        code: "capture_failed".to_string(),
                        message: e,
                    });
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        self.emit(VoiceEvent::End {});
    }

    fn on_frame(&mut self, frame: &[f32]) {
        // Pre-roll always runs so SpeechStart loses nothing (~300 ms ring).
        if !self.capturing {
            self.preroll.extend(frame.iter().copied());
            while self.preroll.len() > FRAME_SAMPLES * 10 {
                self.preroll.pop_front();
            }
        }
        match self.vad.feed(audio::rms(frame)) {
            VadTransition::Silence => {}
            VadTransition::SpeechStart => {
                self.utter.clear();
                self.utter.extend(self.preroll.drain(..));
                self.utter.extend_from_slice(frame);
                self.set_capturing(true);
            }
            VadTransition::SpeechOngoing => {
                if self.capturing {
                    self.utter.extend_from_slice(frame);
                    if self.utter.len() >= self.max_utter_samples {
                        self.finalize();
                    }
                }
            }
            VadTransition::SpeechEnd => {
                if self.capturing {
                    self.utter.extend_from_slice(frame);
                    self.finalize();
                }
            }
        }
    }

    fn finish_capture(&mut self) -> Vec<f32> {
        self.set_capturing(false);
        std::mem::take(&mut self.utter)
    }

    fn finalize(&mut self) {
        let pcm = self.finish_capture();
        if self.stopped() {
            return;
        }
        let text = match self.engine.transcribe(&pcm, &self.opts.lang) {
            Ok(t) => t,
            Err(e) => {
                crate::diagnostics::push(format!("voice: transcription failed: {e}"));
                self.emit(VoiceEvent::Error {
                    code: "transcribe_failed".to_string(),
                    message: e,
                });
                return;
            }
        };
        if text.is_empty() {
            // Breath, chair, fan: the VAD fired on nothing linguistic.
            return;
        }
        match self.opts.mode {
            ListenMode::Manual => {
                self.emit(VoiceEvent::Transcript { text });
                // Single utterance per press: the frontend sends it and the
                // session is done. Nobody presses anything to finish.
                self.emit(VoiceEvent::End {});
                self.stop.store(true, Ordering::Relaxed);
            }
            ListenMode::Wake => match self.arm {
                Arm::AwaitingWake => {
                    if contains_wake_word(&text, &self.opts.wake_word) {
                        crate::diagnostics::push(format!(
                            "voice: wake word heard ({})",
                            self.opts.wake_word
                        ));
                        self.emit(VoiceEvent::Wake {
                            word: self.opts.wake_word.clone(),
                        });
                        self.arm = Arm::AwaitingCommand;
                    }
                    // Else: background chatter, re-arm silently.
                }
                Arm::AwaitingCommand => {
                    self.emit(VoiceEvent::Transcript { text });
                    self.arm = Arm::AwaitingWake;
                }
            },
        }
    }
}
