//! Harness: the sandbox between the model and the machine.
//!
//! The model never touches the OS directly. It sees a [`context::SystemContext`]
//! (OS, session, focused app, input capabilities), declares intent through the
//! [`tools`] catalog (JSON schemas), and the [`agent`] loop executes approved
//! calls via [`exec`] — recording every step as an [`agent::TraceStep`] and,
//! for mutating tools, as a persisted Action row (see chat routes).
//!
//! Layers (each testable in isolation):
//! ```text
//! model ⇄ agent::run_loop ⇄ exec::execute ⇄ OS (enigo / Command / sysinfo)
//!              │
//!              └─ prompt::{system_prompt} + context::{gather}
//! ```
pub mod agent;
pub mod context;
pub mod exec;
pub mod platform;
pub mod prompt;
pub mod tools;
