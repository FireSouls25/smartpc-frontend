//! Tool catalog: the ONLY actions the model may request.
//! Each tool carries its JSON schema (sent to tool-capable models) and a
//! [`Risk`] the executor enforces. Adding a tool = one entry here + one
//! match arm in exec — the model can never reach past this list.
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Pure reads: always allowed.
    ReadOnly,
    /// Reversible, everyday actions (open an app, media keys).
    Low,
    /// Visible side effects (key presses outside the allowlist scope).
    Medium,
    /// Hard to undo or sensitive (typing text): policy-gated.
    High,
}

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
    pub risk: Risk,
    /// Whether a successful call becomes an Action row (left pane).
    /// Read-only tools stay in the run trace only.
    pub records_action: bool,
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

pub fn catalog() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "get_system_context",
            description: "Re-read the machine context: OS and version, CPU, memory, session, focused app, input capabilities. Use for hardware and system questions (processor, memory, OS, focused app) and when the situation may have changed since the run started.",
            parameters: schema(json!({}), &[]),
            risk: Risk::ReadOnly,
            records_action: false,
        },
        ToolDef {
            name: "list_processes",
            description: "List running processes (pid, name, executable) to find an app or check state. Read-only.",
            parameters: schema(
                json!({ "limit": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Max entries (default 30)" } }),
                &[],
            ),
            risk: Risk::ReadOnly,
            records_action: false,
        },
        ToolDef {
            name: "open_app",
            description: "Launch an application by name (e.g. firefox, code, Calculator). No arguments, no URLs, no shell — just the app name.",
            parameters: schema(
                json!({ "name": { "type": "string", "description": "Application name or binary (no paths, no flags)" } }),
                &["name"],
            ),
            risk: Risk::Low,
            records_action: true,
        },
        ToolDef {
            name: "press_key",
            description: "Press one safe key (media controls, navigation, F5/Escape/Tab/Enter/Space). For anything else, explain why and stop.",
            parameters: schema(
                json!({ "key": { "type": "string", "enum": [
                    "play_pause", "next", "prev", "mute",
                    "volume_up", "volume_down",
                    "escape", "tab", "enter", "space",
                    "left", "right", "up", "down", "f5",
                ] } }),
                &["key"],
            ),
            risk: Risk::Medium,
            records_action: true,
        },
        ToolDef {
            name: "type_text",
            description: "Type short text into the focused app (max 500 chars). NEVER into password/secret fields. Often disabled by policy — honor the error and tell the user.",
            parameters: schema(
                json!({ "text": { "type": "string", "maxLength": 500 } }),
                &["text"],
            ),
            risk: Risk::High,
            records_action: true,
        },
    ]
}

/// OpenAI function-tool array, sent verbatim to tool-capable providers.
pub fn openai_schemas() -> Vec<Value> {
    catalog()
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_are_well_formed() {
        let schemas = openai_schemas();
        assert_eq!(schemas.len(), catalog().len());
        for s in &schemas {
            assert_eq!(s["type"], "function");
            assert!(s["function"]["name"].is_string());
            assert_eq!(s["function"]["parameters"]["type"], "object");
        }
    }
}
