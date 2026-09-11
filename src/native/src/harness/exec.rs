//! Tool executor: the sandbox's enforcement point.
//!
//! Unknown tools, malformed args, off-allowlist keys and policy-blocked
//! high-risk calls all fail here as *results* — never panics, never hidden
//! side effects. Blocking OS work runs in `spawn_blocking` so the async
//! runtime stays responsive.
use serde::Serialize;

use super::tools::{catalog, Risk};

#[derive(Debug, Clone)]
pub struct Policy {
    /// High-risk tools (type_text) only run with HARNESS_ALLOW_RISKY=1.
    pub allow_risky: bool,
}

impl Policy {
    pub fn from_env() -> Self {
        Self {
            allow_risky: std::env::var("HARNESS_ALLOW_RISKY").ok().as_deref() == Some("1"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolOutcome {
    pub ok: bool,
    pub output: String,
}

fn ok(output: impl Into<String>) -> ToolOutcome {
    ToolOutcome {
        ok: true,
        output: output.into(),
    }
}

fn err(output: impl Into<String>) -> ToolOutcome {
    ToolOutcome {
        ok: false,
        output: output.into(),
    }
}

fn arg_str(args: &serde_json::Value, key: &str) -> Result<String, ToolOutcome> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| err(format!("missing or invalid string argument: {key}")))
}

pub async fn execute(name: &str, args: &serde_json::Value, policy: &Policy) -> ToolOutcome {
    let risk = match catalog().iter().find(|t| t.name == name) {
        Some(d) => d.risk,
        None => return err(format!("unknown tool: {name}")),
    };
    if risk == Risk::High && !policy.allow_risky {
        return err(
            "type_text is disabled by policy (operator must set HARNESS_ALLOW_RISKY=1); tell the user and stop",
        );
    }
    let name = name.to_string();
    let args = args.clone();
    let allow_risky = policy.allow_risky;
    tokio::task::spawn_blocking(move || {
        let policy = Policy { allow_risky };
        match name.as_str() {
            "get_system_context" => tool_context(),
            "list_processes" => tool_processes(&args),
            "open_app" => tool_open_app(&args),
            "press_key" => tool_press_key(&args),
            "type_text" => tool_type_text(&args, &policy),
            _ => err(format!("unknown tool: {name}")),
        }
    })
    .await
    .unwrap_or_else(|e| err(format!("tool task failed: {e}")))
}

fn tool_context() -> ToolOutcome {
    match serde_json::to_string(&super::context::gather()) {
        Ok(json) => ok(json),
        Err(e) => err(format!("context failed: {e}")),
    }
}

fn tool_processes(args: &serde_json::Value) -> ToolOutcome {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(30)
        .clamp(1, 50) as usize;
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut procs: Vec<serde_json::Value> = sys
        .processes()
        .values()
        .map(|p| {
            serde_json::json!({
                "pid": p.pid().as_u32(),
                "name": p.name().to_string_lossy(),
                "exe": p.exe().and_then(|x| x.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            })
        })
        .collect();
    procs.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    procs.truncate(limit);
    ok(serde_json::Value::Array(procs).to_string())
}

fn tool_open_app(args: &serde_json::Value) -> ToolOutcome {
    let name = match arg_str(args, "name") {
        Ok(n) => n,
        Err(e) => return e,
    };
    if name.len() > 128 {
        return err("app name too long");
    }
    if name.contains("..")
        || name.chars().any(|c| {
            matches!(
                c,
                '/' | '\\'
                    | ';'
                    | '&'
                    | '|'
                    | '$'
                    | '`'
                    | '~'
                    | '('
                    | ')'
                    | '<'
                    | '>'
                    | '"'
                    | '\''
                    | '*'
                    | '?'
                    | '!'
            ) || c.is_control()
        })
    {
        return err("app name must be a plain name (no paths, flags or shell characters)");
    }
    spawn_detached(&name)
}

#[cfg(target_os = "macos")]
fn spawn_detached(name: &str) -> ToolOutcome {
    use std::process::Stdio;
    match std::process::Command::new("open")
        .arg("-a")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => ok(format!("started {name} (pid {})", child.id())),
        Err(_) => spawn_path(name),
    }
}

#[cfg(target_os = "windows")]
fn spawn_detached(name: &str) -> ToolOutcome {
    use std::process::Stdio;
    match std::process::Command::new("cmd")
        .args(["/C", "start", "", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => ok(format!("started {name} (pid {})", child.id())),
        Err(e) => err(format!("could not start {name}: {e}")),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn spawn_detached(name: &str) -> ToolOutcome {
    spawn_path(name)
}

#[cfg(not(target_os = "windows"))]
fn spawn_path(name: &str) -> ToolOutcome {
    use std::process::Stdio;
    match std::process::Command::new(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => ok(format!("started {name} (pid {})", child.id())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            err(format!("{name} not found in PATH or as a known app"))
        }
        Err(e) => err(format!("could not start {name}: {e}")),
    }
}

fn enigo_key(name: &str) -> Option<enigo::Key> {
    use enigo::Key::*;
    Some(match name {
        "escape" => Escape,
        "tab" => Tab,
        "enter" => Return,
        "space" => Space,
        "left" => LeftArrow,
        "right" => RightArrow,
        "up" => UpArrow,
        "down" => DownArrow,
        "f5" => F5,
        "play_pause" => MediaPlayPause,
        "next" => MediaNextTrack,
        "prev" => MediaPrevTrack,
        "mute" => VolumeMute,
        "volume_up" => VolumeUp,
        "volume_down" => VolumeDown,
        _ => return None,
    })
}

fn tool_press_key(args: &serde_json::Value) -> ToolOutcome {
    let key_name = match arg_str(args, "key") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let key = match enigo_key(&key_name) {
        Some(k) => k,
        None => {
            return err(format!(
                "key not allowlisted: {key_name} (ask the user instead)"
            ));
        }
    };
    match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(mut e) => {
            use enigo::Keyboard;
            match e.key(key, enigo::Direction::Click) {
                Ok(()) => ok(format!("pressed {key_name}")),
                Err(er) => err(format!("input failed (display server may gate it): {er}")),
            }
        }
        Err(er) => err(format!(
            "no input backend available: {er} (headless session or Wayland without approval?)"
        )),
    }
}

fn tool_type_text(args: &serde_json::Value, policy: &Policy) -> ToolOutcome {
    // Defense in depth: execute() already gates High risk; re-check here.
    if !policy.allow_risky {
        return err("type_text is disabled by policy");
    }
    let text = match arg_str(args, "text") {
        Ok(t) => t,
        Err(e) => return e,
    };
    if text.chars().count() > 500 {
        return err("text too long (max 500 chars)");
    }
    match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(mut e) => {
            use enigo::Keyboard;
            match e.text(&text) {
                Ok(()) => ok(format!("typed {} chars", text.chars().count())),
                Err(er) => err(format!("input failed (display server may gate it): {er}")),
            }
        }
        Err(er) => err(format!(
            "no input backend available: {er} (headless session or Wayland without approval?)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locked_down() -> Policy {
        Policy { allow_risky: false }
    }

    #[tokio::test]
    async fn rejects_unknown_tools() {
        let out = execute("rm_rf_everything", &serde_json::json!({}), &locked_down()).await;
        assert!(!out.ok);
        assert!(out.output.contains("unknown tool"));
    }

    #[tokio::test]
    async fn validates_open_app_names() {
        for bad in ["", "   ", "../x", "a/b", "x;rm", "x|y", "x$y", "a\"b"] {
            let out = execute(
                "open_app",
                &serde_json::json!({"name": bad}),
                &locked_down(),
            )
            .await;
            assert!(!out.ok, "{bad:?} should be rejected");
        }
    }

    #[tokio::test]
    async fn rejects_off_allowlist_keys() {
        let out = execute(
            "press_key",
            &serde_json::json!({"key": "super_secret_combo"}),
            &locked_down(),
        )
        .await;
        assert!(!out.ok);
        assert!(out.output.contains("allowlisted"));
    }

    #[tokio::test]
    async fn risky_tools_stay_gated() {
        let out = execute(
            "type_text",
            &serde_json::json!({"text": "hello"}),
            &locked_down(),
        )
        .await;
        assert!(!out.ok);
        assert!(out.output.contains("policy"));
    }

    #[tokio::test]
    async fn context_and_processes_are_readable() {
        let ctx = execute("get_system_context", &serde_json::json!({}), &locked_down()).await;
        assert!(ctx.ok, "context failed: {}", ctx.output);
        assert!(ctx.output.contains("\"os\""));
        let procs = execute(
            "list_processes",
            &serde_json::json!({"limit": 2}),
            &locked_down(),
        )
        .await;
        assert!(procs.ok, "processes failed: {}", procs.output);
        let arr: serde_json::Value = serde_json::from_str(&procs.output).unwrap();
        assert!(arr.as_array().unwrap().len() <= 2);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn open_app_spawns_path_binaries() {
        // /usr/bin/true exits instantly: proves spawn plumbing, harms nothing.
        let out = execute(
            "open_app",
            &serde_json::json!({"name": "true"}),
            &locked_down(),
        )
        .await;
        assert!(out.ok, "spawn failed: {}", out.output);
        assert!(out.output.contains("started true (pid "));
    }
}
