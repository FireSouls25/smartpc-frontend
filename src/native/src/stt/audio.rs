//! Mic capture (cpal) + PCM plumbing to 16 kHz mono f32, whisper's diet.
//!
//! One code path everywhere: default input → whatever rate/channels the OS
//! gives → mix down + linear-resample in the callback. Callbacks must never
//! block, so full 30 ms frames go through a bounded channel (overflow drops,
//! underruns can't happen — the VAD just sees a gap).
use std::sync::mpsc::SyncSender;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::vad::FRAME_SAMPLES;

/// Whisper's native rate. Everything converges here.
pub const TARGET_RATE: u32 = 16000;

#[derive(Debug)]
pub enum CaptureError {
    NoMicrophone,
    Unsupported(String),
    Stream(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMicrophone => write!(f, "no microphone found"),
            Self::Unsupported(e) => write!(f, "unsupported audio config: {e}"),
            Self::Stream(e) => write!(f, "audio stream failed: {e}"),
        }
    }
}

pub fn microphone_present() -> bool {
    cpal::default_host().default_input_device().is_some()
}

/// Human-readable input device name for diagnostics ("are we listening to
/// the right mic?"). None when there is no input device at all.
pub fn input_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.description().ok())
        .map(|desc| desc.name().to_string())
        .filter(|n| !n.trim().is_empty())
}

/// All input device names (for the settings picker). Empty when the host
/// reports none — never an error: absence is data, not failure.
pub fn list_input_devices() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|devices| {
            devices
                .filter_map(|d| d.description().ok())
                .map(|desc| desc.name().to_string())
                .filter(|n| !n.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// What the opened stream actually runs at (logged: sample-rate surprises
/// are a classic "VAD hears nothing useful" cause).
#[derive(Debug, Clone)]
pub struct CaptureDesc {
    pub rate: u32,
    pub channels: usize,
    pub format: String,
}

pub fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f32 = frame.iter().map(|s| s * s).sum();
    (sum / frame.len() as f32).sqrt()
}

/// Linear resample mono f32. Good enough for voice (whisper is robust to
/// resampling artifacts); avoids pulling a DSP crate for one call site.
pub fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = ((input.len() as f64) / ratio).ceil() as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let i0 = pos.floor() as usize;
            let frac = (pos - i0 as f64) as f32;
            let s0 = input.get(i0).copied().unwrap_or(0.0);
            let s1 = input.get(i0 + 1).copied().unwrap_or(s0);
            s0 + (s1 - s0) * frac
        })
        .collect()
}

fn i16_to_f32(v: i16) -> f32 {
    v as f32 / 32768.0
}

fn u16_to_f32(v: u16) -> f32 {
    v as f32 / 32768.0 - 1.0
}

/// Batches device-rate mono into 16 kHz 480-sample frames. One per stream
/// callback closure (owned, `'static`); overflow drops, never blocks audio.
struct Framer {
    tx: SyncSender<Result<Vec<f32>, String>>,
    leftover: Vec<f32>,
    from_rate: u32,
}

impl Framer {
    fn push(&mut self, mono_at_device_rate: &[f32]) {
        self.leftover.extend(resample_linear(
            mono_at_device_rate,
            self.from_rate,
            TARGET_RATE,
        ));
        while self.leftover.len() >= FRAME_SAMPLES {
            let frame: Vec<f32> = self.leftover.drain(..FRAME_SAMPLES).collect();
            let _ = self.tx.try_send(Ok(frame));
        }
    }
}

fn framer(tx: &SyncSender<Result<Vec<f32>, String>>, from_rate: u32) -> Framer {
    Framer {
        tx: tx.clone(),
        leftover: Vec::with_capacity(FRAME_SAMPLES * 4),
        from_rate,
    }
}

/// Open a mic for capture. `wanted`: `None`/empty = system default,
/// otherwise an exact device name from [`list_input_devices`].
/// Frames arrive as `Ok([f32; 480])`; stream failures arrive as
/// `Err(message)` and the caller must shut down.
pub fn open_capture(
    tx: SyncSender<Result<Vec<f32>, String>>,
    wanted: Option<&str>,
) -> Result<(cpal::Stream, CaptureDesc), CaptureError> {
    let host = cpal::default_host();
    let device = match wanted.filter(|w| !w.trim().is_empty()) {
        Some(name) => host
            .input_devices()
            .map_err(|e| CaptureError::Unsupported(e.to_string()))?
            .find(|d| {
                d.description()
                    .is_ok_and(|desc| desc.name() == name)
            })
            .ok_or_else(|| {
                CaptureError::Unsupported(format!("input device not found: {name}"))
            })?,
        None => host.default_input_device().ok_or(CaptureError::NoMicrophone)?,
    };
    let desc = device
        .description()
        .ok()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|| "?".to_string());
    let config = device
        .default_input_config()
        .map_err(|e| CaptureError::Unsupported(e.to_string()))?;
    let from_rate = config.sample_rate();
    let channels = config.channels() as usize;
    if channels == 0 {
        return Err(CaptureError::Unsupported("0 channels".to_string()));
    }
    let stream_config: cpal::StreamConfig = config.config();
    let format = format!("{:?}", config.sample_format());
    let err_tx = tx.clone();
    let err_fn = move |err| {
        let _ = err_tx.send(Err(format!("{err}")));
    };
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut framer = framer(&tx, from_rate);
            device.build_input_stream(
                stream_config.clone(),
                move |data: &[f32], _| {
                    framer.push(&mix_down(data, channels));
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let mut framer = framer(&tx, from_rate);
            device.build_input_stream(
                stream_config.clone(),
                move |data: &[i16], _| {
                    let conv: Vec<f32> =
                        data.iter().map(|v| i16_to_f32(*v)).collect();
                    framer.push(&mix_down(&conv, channels));
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let mut framer = framer(&tx, from_rate);
            device.build_input_stream(
                stream_config.clone(),
                move |data: &[u16], _| {
                    let conv: Vec<f32> =
                        data.iter().map(|v| u16_to_f32(*v)).collect();
                    framer.push(&mix_down(&conv, channels));
                },
                err_fn,
                None,
            )
        }
        f => {
            return Err(CaptureError::Unsupported(format!(
                "sample format {f:?}"
            )));
        }
    }
    .map_err(|e| CaptureError::Stream(e.to_string()))?;
    stream
        .play()
        .map_err(|e| CaptureError::Stream(e.to_string()))?;
    Ok((
        stream,
        CaptureDesc {
            rate: from_rate,
            channels,
            format,
        },
    ))
}

/// Mix N channels to mono, then resample to 16 kHz. `data` is interleaved.
fn mix_down(data: &[f32], channels: usize) -> Vec<f32> {
    if data.is_empty() {
        return Vec::new();
    }
    let frames = data.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for i in 0..frames {
        let mut sum = 0.0;
        for c in 0..channels {
            sum += data[i * channels + c];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;

    #[test]
    fn rms_measures_energy() {
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[0.5, -0.5, 0.5, -0.5]) - 0.5).abs() < 1e-6);
        assert!(rms(&[0.0; 480]) < 1e-9);
    }

    #[test]
    fn resample_is_identity_at_target_rate() {
        let v = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_linear(&v, 16000, 16000), v);
        assert!(resample_linear(&[], 48000, 16000).is_empty());
    }

    #[test]
    fn resample_halves_length_and_keeps_endpoints() {
        let v: Vec<f32> = (0..480).map(|i| i as f32 / 480.0).collect();
        let out = resample_linear(&v, 48000, 16000);
        assert_eq!(out.len(), 160);
        assert!((out[0] - 0.0).abs() < 1e-6);
        // Linear interp lands within one input step of the true ramp.
        for (i, s) in out.iter().enumerate() {
            let want = (i * 3) as f32 / 480.0;
            assert!((s - want).abs() < 1.0 / 480.0 + 1e-6, "{i}: {s} vs {want}");
        }
    }

    #[test]
    fn mix_down_averages_channels() {
        assert_eq!(mix_down(&[1.0, 3.0, 2.0, 4.0], 2), vec![2.0, 3.0]);
        assert_eq!(mix_down(&[0.5, 0.5], 1), vec![0.5, 0.5]);
        assert!(mix_down(&[], 2).is_empty());
    }

    #[test]
    fn sample_conversions_span_minus_one_to_one() {
        assert!((i16_to_f32(32767) - 1.0).abs() < 1e-4);
        assert!((i16_to_f32(-32768) + 1.0).abs() < 1e-4);
        assert!((u16_to_f32(65535) - 1.0).abs() < 1e-4);
        assert!((u16_to_f32(0) + 1.0).abs() < 1e-4);
    }

    #[test]
    fn channel_capacity_constant_is_sane() {
        let (tx, rx) = sync_channel::<Result<Vec<f32>, String>>(128);
        tx.try_send(Ok(vec![0.0; FRAME_SAMPLES])).unwrap();
        assert_eq!(rx.recv().unwrap().unwrap().len(), FRAME_SAMPLES);
    }
}
