//! Local voice input: mic → energy VAD → whisper.cpp → text.
//!
//! Pipeline (all on-device, no network after the first model download):
//!
//! ```text
//! cpal mic ──► 16 kHz mono frames ──► VAD (speech? silence?) ──► utterance
//!                                                              │
//!                              wake mode: transcribe onset ──► wake word?
//!                                                              │ yes
//!                                                              ▼
//!                                              transcribe command ──► event
//! manual mode: first utterance ──► transcribe ──► event + auto-stop
//! ```
//!
//! The frontend long-polls `events` and feeds final transcripts straight
//! into the agent (they render as normal user messages). Silence finalizes:
//! nobody presses anything — or says anything — to *finish*. Starting is
//! either the mic button (manual) or the wake word ("hey" default,
//! configurable) in wake mode.
//!
//! [`session::VoiceService`] owns the single global session; see
//! `docs/09-voice.md` for models, tuning knobs, platforms and limits.
pub mod audio;
pub mod engine;
pub mod model;
pub mod routes;
pub mod session;
pub mod vad;
pub mod wake;

pub use session::VoiceService;
