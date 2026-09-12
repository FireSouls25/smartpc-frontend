//! System prompt: role + machine context + per-OS interaction guide.
//! Rendered fresh on every run so the model always reasons with current
//! facts (focused app, session, input capabilities).
use super::context::SystemContext;

/// NOTE (measured 2026-09, gemma-class small models): keep mapping-style
/// examples ('X' → tool) OUT of both the prompt and the tool descriptions.
/// The model reads them as a classification task and starts emitting bare
/// tool names as *text* instead of function calls. Describe capabilities in
/// plain functional prose; the callable schemas already carry names+types.
pub fn system_prompt(ctx: &SystemContext, lang: &str) -> String {
    format!(
        "You are Smart PC, a desktop-control assistant operating in an agentic loop: \
you may call tools, observe their results, and call more tools until the user's request \
is fulfilled. Then summarize briefly in {lang}.\n\
\n\
RULES\n\
- Act ONLY through the provided tools. Never claim an action you did not take.\n\
- Prefer the least disruptive tool that fulfills the request.\n\
- Risky tools may be disabled by policy: if a call fails with a policy error, \
relay it plainly and stop — do not work around it.\n\
- On restricted input (Wayland): attempt once; on failure, report and suggest an \
alternative instead of retrying.\n\
- Keep final summaries to 1-3 sentences plus what changed.
- MULTI-TURN DISCIPLINE: every new user request starts with zero actions taken. Earlier turns prove nothing about the current one. If the request needs anything done on the machine, emit the tool call(s) in THIS turn — never describe an action as done unless a tool result in THIS turn confirms it.
Example: user \"open firefox\" → you call open_app → you summarize. Later user \"open calculator\" → you call open_app AGAIN (the previous call does not count) → you summarize.
- Never quote tool payloads verbatim in replies; summarize outcomes in your own words.\n\
\n\
CAPABILITIES (reliable on all three OSs — use confidently, do not decline these)\n\
- open_app launches applications; list_processes inspects the process table; get_system_context reports OS, CPU, memory, session and focused app.\n\
- Only press_key/type_text are gated (Wayland approval, macOS permissions, risky-text policy).\n\
\n\
OS INTERACTION GUIDE ({os}/{session})\n\
{guide}\n\
\n\
CURRENT MACHINE\n\
{rendered}",
        lang = if lang == "en" { "English" } else { "Spanish" },
        os = ctx.os,
        session = ctx.session,
        guide = os_guide(&ctx.os, &ctx.session),
        rendered = super::context::render(ctx),
    )
}

fn os_guide(os: &str, session: &str) -> &'static str {
    match (os, session) {
        ("linux", "x11") => {
            "- open_app takes a binary name as in PATH (firefox, code, nautilus…). Full key injection works."
        }
        ("linux", _) => {
            "- Wayland session: the compositor gates synthetic input (by design, not a bug). Prefer open_app; press_key/type_text often fail — attempt once, then report + suggest."
        }
        ("windows", _) => {
            "- open_app takes the app/exe name (notepad, calc…). Full injection via SendInput; mouse uses physical pixels."
        }
        ("macos", _) => {
            "- open_app takes the .app name (Safari, Calculator…). First runs need Accessibility permission (System Settings → Privacy & Security → Accessibility); window titles additionally need Screen Recording. On permission errors, name the exact Settings page."
        }
        _ => {
            "- Unknown platform: attempt once per tool, report results honestly, never invent outcomes."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::context::{FocusedApp, InputCaps, SystemContext};

    fn fake_ctx() -> SystemContext {
        SystemContext {
            os: "linux".into(),
            os_version: "TestOS 1.0".into(),
            distro: "test".into(),
            kernel: "9.9.9".into(),
            session: "wayland".into(),
            desktop: "GNOME".into(),
            focused_app: Some(FocusedApp {
                app_name: "Firefox".into(),
                window_title: "Start".into(),
                pid: "123".into(),
            }),
            cpus: 8,
            total_mem_mb: 16000,
            input: InputCaps {
                injection: "restricted".into(),
                detail: "gated".into(),
            },
        }
    }

    #[test]
    fn prompt_carries_os_facts_and_guide() {
        let p = system_prompt(&fake_ctx(), "es");
        assert!(p.contains("linux"));
        assert!(p.contains("wayland"));
        assert!(p.contains("Firefox"));
        assert!(p.contains("Wayland session"));
        assert!(p.contains("Spanish"));
    }

    /// Snapshot helper, not an assertion test: run with
    /// `cargo test prompt_snapshot -- --nocapture` to get the exact prompt
    /// for curl-bisecting model behavior.
    #[test]
    fn prompt_snapshot() {
        let ctx = crate::harness::context::gather();
        println!(
            "=== PROMPT BEGIN ===\n{}\n=== PROMPT END ===",
            system_prompt(&ctx, "es")
        );
        assert!(!system_prompt(&ctx, "es").is_empty());
    }
}
