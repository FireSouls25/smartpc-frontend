//! Platform specifics behind one cross-platform surface.
//!
//! Research notes (kept here so future ports don't re-learn them):
//! - Display session comes from the environment, no syscalls needed.
//! - Focused window uses active-win-pos-rs on all three OSs: X11/XCB,
//!   KDE (kdotool) and Hyprland on Wayland, Win32 foreground window,
//!   NSWorkspace on macOS (title needs Screen Recording permission there).
//! - sysinfo deliberately does NOT do foreground windows — hence this module.
use super::context::FocusedApp;

/// "x11" | "wayland" on Linux; the OS name itself on Windows/macOS so the
/// prompt and capability matrix stay uniform ("unknown" when undetectable).
pub fn display_session() -> String {
    if cfg!(target_os = "windows") {
        return "windows".into();
    }
    if cfg!(target_os = "macos") {
        return "macos".into();
    }
    match std::env::var("XDG_SESSION_TYPE")
        .map(|v| v.to_lowercase())
        .as_deref()
    {
        Ok("wayland") => "wayland",
        Ok("x11") => "x11",
        _ if std::env::var_os("WAYLAND_DISPLAY").is_some() => "wayland",
        _ if std::env::var_os("DISPLAY").is_some() => "x11",
        _ => "unknown",
    }
    .to_string()
}

/// Best-effort focused app. `None` is a normal answer (headless session,
/// permission missing, unsupported compositor) — callers must not fail.
pub fn focused_app() -> Option<FocusedApp> {
    let w = active_win_pos_rs::get_active_window().ok()?;
    let app_name = w.app_name.trim().to_string();
    let window_title = w.title.trim().to_string();
    if app_name.is_empty() && window_title.is_empty() {
        return None;
    }
    Some(FocusedApp {
        app_name,
        window_title,
        pid: w.process_id.to_string(),
    })
}
