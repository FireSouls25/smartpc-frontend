//! Pi harness (embedded RPC): pi reasons, the sidecar disposes.
//!
//! When `PI_HARNESS=1`, chat turns run through `pi --mode rpc` (one node
//! child per sidecar user) instead of the native agent loop. The UI, the
//! HTTP contract, sessions/actions storage, auth, keys and the risk sandbox
//! are unchanged — only the reasoning engine swaps:
//!
//! ```text
//! renderer ──HTTP──► sidecar ──stdio JSONL──► pi --mode rpc
//!                        │                          │
//!                        │ tools (bridge ext, -e)   │ execute() callback
//!                        │◄── POST /internal/pi/tool ┘
//!                        └── SQLite, policy, keys (unchanged)
//! ```
//!
//! Layers:
//! - [`supervisor`] spawns/supervises pi children (one per user + a keyless
//!   system child for catalog queries), strict-`\n` JSONL framing, id
//!   correlation, per-user turn serialization, model catalog cache.
//! - [`turn`] runs one agentic turn (session mapping, set_model, prompt to
//!   `agent_settled`) and maps events to reply/steps/actions/context.
//! - [`providers`] maps pi's model catalog to our `ProviderInfo` shape.
//! - [`routes`] serves the bridge endpoints the TS extension calls
//!   (`/internal/pi/tools`, `/internal/pi/bootstrap`, `/internal/pi/tool`).
//!
//! [`enabled`] gates everything: unset/≠1 keeps the native harness.
pub mod providers;
pub mod routes;
pub mod supervisor;
pub mod turn;

pub use supervisor::PiSupervisor;

/// Master switch. Read per turn (cheap) so tests and debugging can flip it
/// without a restart narrative.
pub fn enabled() -> bool {
    std::env::var("PI_HARNESS").ok().as_deref() == Some("1")
}

/// Pinned pi distribution for `npx -p`. Bump deliberately (also in docs):
/// the bridge is tested against exactly this version.
pub const PI_PACKAGE: &str = "@earendil-works/pi-coding-agent@0.87.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_defaults_off() {
        let prev = std::env::var("PI_HARNESS").ok();
        std::env::remove_var("PI_HARNESS");
        assert!(!enabled());
        match prev {
            Some(v) => std::env::set_var("PI_HARNESS", v),
            None => std::env::remove_var("PI_HARNESS"),
        }
    }
}
