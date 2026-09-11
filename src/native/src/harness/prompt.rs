//! System prompt: role + machine context + per-OS interaction guide.
//! Rendered fresh on every run so the model always reasons with current
//! facts (focused app, session, input capabilities).
use super::context::SystemContext;

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
- Keep final summaries to 1-3 sentences plus what changed.\n\
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
}
