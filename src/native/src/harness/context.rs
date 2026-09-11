//! Machine context the model reasons with: OS, session, focused app,
//! input capabilities. Small by design — only what changes decisions
//! crosses into the prompt (sysinfo + one active-window probe).
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FocusedApp {
    pub app_name: String,
    pub window_title: String,
    pub pid: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputCaps {
    /// "full" | "restricted" | "unknown"
    pub injection: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemContext {
    pub os: String,
    pub os_version: String,
    pub distro: String,
    pub kernel: String,
    pub session: String,
    pub desktop: String,
    pub focused_app: Option<FocusedApp>,
    pub cpus: usize,
    pub total_mem_mb: u64,
    pub input: InputCaps,
}

fn input_caps(os: &str, session: &str) -> InputCaps {
    match (os, session) {
        ("linux", "x11") => InputCaps {
            injection: "full".into(),
            detail: "synthetic input via enigo/X11 (key injection, text, mouse)".into(),
        },
        ("linux", _) => InputCaps {
            injection: "restricted".into(),
            detail: "Wayland compositors gate synthetic input: prefer open_app and
window-agnostic actions; press_key/type_text often fail — attempt once,
report honestly, never retry in a loop"
                .into(),
        },
        ("windows", _) => InputCaps {
            injection: "full".into(),
            detail: "Win32 SendInput (physical pixels for mouse)".into(),
        },
        ("macos", _) => InputCaps {
            injection: "full".into(),
            detail: "CGEvent — needs Accessibility permission; window titles need
Screen Recording permission; on permission errors tell the user the exact
macOS Settings page to open"
                .into(),
        },
        _ => InputCaps {
            injection: "unknown".into(),
            detail: "unprobed platform — attempt once, report honestly".into(),
        },
    }
}

pub fn gather() -> SystemContext {
    let sys = sysinfo::System::new_all();
    let os = std::env::consts::OS.to_string();
    let session = super::platform::display_session();
    let input = input_caps(&os, &session);
    SystemContext {
        os_version: sysinfo::System::long_os_version()
            .or_else(sysinfo::System::os_version)
            .unwrap_or_default(),
        distro: sysinfo::System::distribution_id(),
        kernel: sysinfo::System::kernel_version().unwrap_or_default(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        focused_app: super::platform::focused_app(),
        cpus: sys.cpus().len(),
        total_mem_mb: sys.total_memory() / 1024 / 1024,
        os,
        session,
        input,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_gathers_and_renders() {
        let ctx = gather();
        assert!(!ctx.os.is_empty());
        assert!(!ctx.session.is_empty());
        // Focused app may be absent (headless/quantum) — must not fail.
        let rendered = render(&ctx);
        assert!(rendered.contains(&ctx.os));
        assert!(rendered.contains(&ctx.session));
    }
}

/// Compact rendering for the system prompt.
pub fn render(ctx: &SystemContext) -> String {
    let focused = match &ctx.focused_app {
        Some(a) => format!(
            "{} (pid {}, window {:?})",
            a.app_name, a.pid, a.window_title
        ),
        None => "unknown".to_string(),
    };
    format!(
        "OS: {os} {ver} ({distro}) · kernel {kernel}\n\
         session: {session} · desktop: {desktop}\n\
         focused app: {focused}\n\
         input injection: {inj} — {detail}",
        os = ctx.os,
        ver = ctx.os_version,
        distro = if ctx.distro.is_empty() {
            "-"
        } else {
            &ctx.distro
        },
        kernel = ctx.kernel,
        session = ctx.session,
        desktop = if ctx.desktop.is_empty() {
            "-"
        } else {
            &ctx.desktop
        },
        focused = focused,
        inj = ctx.input.injection,
        detail = ctx.input.detail,
    )
}
