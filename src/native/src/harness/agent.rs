//! Agent loop: the model decides, the harness disposes.
//!
//! Each iteration sends history + tool schemas; the model either answers
//! (done) or emits `tool_calls`, which run through [`exec::execute`] with
//! results fed back as `tool` messages. Bounded by `max_iters`; providers
//! that reject the `tools` array get exactly one plain retry.
use serde::Serialize;

use super::{
    exec::{execute, Policy},
    tools,
};
use crate::ai::provider::{
    AssistantToolCall, ChatMessage, ChatOptions, ChatResponse, FunctionRef, LlmProvider,
    ProviderError, Role, ToolArgs, ToolCall, ToolChatResponse, ToolFunction,
};

#[derive(Debug, Clone, Serialize)]
pub struct TraceStep {
    pub tool: String,
    pub args: serde_json::Value,
    pub action_id: Option<String>,
    pub ok: bool,
    pub output_preview: String,
}

/// Where mutating-tool executions get recorded. Read-only tools skip the
/// Action row (they stay in the trace only); return `None` for those.
pub trait ActionSink: Send + Sync {
    fn action_started(&self, kind: &str, title: &str) -> Option<String>;
    fn action_finished(&self, action_id: &str, ok: bool);
}

pub(crate) fn title_for(tool: &str, args: &serde_json::Value) -> String {
    let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("?");
    match tool {
        "open_app" => format!("Abrir {}", arg("name")),
        "close_app" => format!("Cerrar {}", arg("name")),
        "press_key" => format!("Pulsar {}", arg("key")),
        "type_text" => "Escribir texto".to_string(),
        _ => tool.to_string(),
    }
}

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn preview(s: &str) -> String {
    truncate_chars(s, 300)
}

fn debug_enabled() -> bool {
    std::env::var("HARNESS_DEBUG").ok().as_deref() == Some("1")
}

fn assistant_echo(text: &str, calls: &[crate::ai::provider::ToolCall]) -> ChatMessage {
    ChatMessage {
        role: Role::Assistant,
        content: text.to_string(),
        tool_calls: Some(
            calls
                .iter()
                .map(|c| AssistantToolCall {
                    id: c.id.clone(),
                    kind: Some("function".into()),
                    function: FunctionRef {
                        name: c.function.name.clone(),
                        arguments: c.function.arguments.as_string(),
                    },
                })
                .collect(),
        ),
    }
}

fn tool_message(
    call_id: &Option<String>,
    name: &str,
    outcome: &super::exec::ToolOutcome,
) -> ChatMessage {
    let mut content = serde_json::json!({
        "tool": name,
        "ok": outcome.ok,
        "output": outcome.output,
    });
    if let Some(id) = call_id {
        content["tool_call_id"] = serde_json::Value::String(id.clone());
    }
    ChatMessage {
        role: Role::Tool,
        content: content.to_string(),
        tool_calls: None,
    }
}

/// Per-turn action reminder, appended to the newest user message (in-memory
/// only, never persisted): small models rest on earlier turns' claims
/// ("already closed it") and answer without acting. Recency beats the
/// system prompt's discipline line. Shared with the pi harness, which has
/// no other per-turn injection channel.
pub(crate) const TURN_REMINDER: &str = "[Reminder: earlier turns prove nothing about the current request. If anything must be done on the machine, emit the tool call(s) in THIS turn — never describe an action as done unless a tool result in THIS turn confirms it.]";

fn inject_turn_reminder(messages: &mut Vec<ChatMessage>) {
    if let Some(last) = messages
        .iter_mut()
        .rev()
        .find(|m| matches!(m.role, Role::User))
    {
        if !last.content.contains("in THIS turn confirms it") {
            last.content.push_str("\n");
            last.content.push_str(TURN_REMINDER);
        }
    }
}

fn is_tools_rejection(e: &ProviderError) -> bool {
    // Any gateway failure on a tools call is worth one plain retry: models
    // without tool support fail with model-specific errors (400 variants,
    // 500s) that never mention the word "tool".
    matches!(e, ProviderError::Status(code, _) if (400..600).contains(code))
}

/// Max fenced calls honored per reply in text mode (runaway guard).
const MAX_TEXT_CALLS: usize = 3;

/// Text-embedded tool calls for models without native function calling.
/// Accepts any ``` fence (```tool, ```json, bare) and both key spellings
/// (`name`/`arguments` and `tool`/`args`): small models paraphrase the
/// format. Unknown tool names and malformed blocks are skipped, never
/// executed. Also returns the reply with consumed blocks removed, so
/// executed calls never leak into user-visible text.
fn extract_text_calls(text: &str) -> (Vec<(String, serde_json::Value)>, String) {
    let known: Vec<&str> = super::tools::catalog().iter().map(|t| t.name).collect();
    let mut calls = vec![];
    // (start, end) byte spans of consumed blocks, cut afterwards.
    let mut spans = vec![];
    // Re-scan after each fence: a malformed block must not swallow later ones.
    let mut rest = text;
    let mut base = 0;
    while let Some(start) = rest.find("```") {
        let fence = base + start;
        let after_open = &rest[start + "```".len()..];
        // The block starts on the next line (whatever tag follows ```).
        let Some(nl) = after_open.find('\n') else {
            break;
        };
        let content_at = fence + "```".len() + nl + 1;
        let content = &text[content_at..];
        let Some(end_rel) = content.find("```") else {
            break;
        };
        rest = &content[end_rel + "```".len()..];
        base = content_at + end_rel + "```".len();
        let block = content[..end_rel].trim();
        let Some((name, args)) = as_text_call(block, &known) else {
            continue;
        };
        spans.push((fence, base));
        calls.push((name, args));
    }
    let mut cleaned = String::with_capacity(text.len());
    let mut cursor = 0;
    for (s, e) in spans {
        cleaned.push_str(&text[cursor..s]);
        cursor = e;
    }
    cleaned.push_str(&text[cursor..]);
    (calls, cleaned)
}

/// Lenient single-block parse: full JSON or first {...} slice, both key
/// spellings, known tool name, object arguments.
fn as_text_call(block: &str, known: &[&str]) -> Option<(String, serde_json::Value)> {
    let v: serde_json::Value = serde_json::from_str(block).ok().or_else(|| {
        let from = block.find('{')?;
        let to = block.rfind('}')?;
        if to <= from {
            return None;
        }
        serde_json::from_str(&block[from..=to]).ok()
    })?;
    let name = v
        .get("name")
        .or_else(|| v.get("tool"))
        .and_then(|n| n.as_str())?;
    if !known.iter().any(|k| *k == name) {
        return None;
    }
    let args = v
        .get("arguments")
        .or_else(|| v.get("args"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    if !args.is_object() {
        return None;
    }
    Some((name.to_string(), args))
}

#[cfg(test)]
fn parse_text_calls(text: &str) -> Vec<(String, serde_json::Value)> {
    extract_text_calls(text).0
}

#[allow(clippy::too_many_arguments)]
pub async fn run_loop<P: LlmProvider>(
    provider: &P,
    model: &str,
    history: Vec<ChatMessage>,
    system: &str,
    sink: &dyn ActionSink,
    policy: &Policy,
    max_iters: usize,
) -> Result<(String, Vec<TraceStep>, Vec<ChatMessage>), ProviderError> {
    let text_mode = !provider.supports_tools(model);
    let system_full = if text_mode {
        format!("{system}\n\n{}", super::prompt::text_tool_guide())
    } else {
        system.to_string()
    };
    let mut messages = Vec::with_capacity(history.len() + 2);
    messages.push(ChatMessage {
        role: Role::System,
        content: system_full,
        tool_calls: None,
    });
    messages.extend(history);
    inject_turn_reminder(&mut messages);

    let opts = ChatOptions {
        model: model.to_string(),
        // Low temperature: agentic runs want instruction compliance
        // (tool discipline), not creativity.
        temperature: Some(0.2),
        max_tokens: None,
        json_mode: false,
    };
    let schemas = tools::openai_schemas();
    let mut trace = Vec::new();
    let mut tool_turns = Vec::new();
    let mut last_text = String::new();
    let mut iters = 0usize;

    loop {
        if debug_enabled() {
            let last_user = messages
                .iter()
                .rev()
                .find(|m| matches!(m.role, Role::User))
                .map(|m| truncate_chars(&m.content, 160))
                .unwrap_or_default();
            eprintln!("[agent:debug] iter={iters} user_msg={last_user:?}");
        }
        if iters >= max_iters {
            if last_text.trim().is_empty() {
                last_text = "Alcancé el límite de pasos con trabajo pendiente.".to_string();
            }
            return Ok((last_text, trace, tool_turns));
        }
        iters += 1;

        // Tool-less models (Zen free tier) get plain chat; their fenced
        // calls parse below into the same downstream shape, so execution,
        // trace and persistence are identical from here on.
        let resp: ToolChatResponse = if text_mode {
            let plain = provider.chat(messages.clone(), &opts).await?;
            let (calls, cleaned) = extract_text_calls(&plain.text);
            ToolChatResponse {
                text: cleaned,
                tool_calls: calls
                    .into_iter()
                    .take(MAX_TEXT_CALLS)
                    .map(|(name, args)| ToolCall {
                        id: None,
                        kind: Some("function".into()),
                        function: ToolFunction {
                            name,
                            arguments: ToolArgs::Obj(args),
                        },
                    })
                    .collect(),
            }
        } else {
            match provider
                .chat_with_tools(messages.clone(), &opts, &schemas)
                .await
            {
                Ok(r) => r,
                Err(e) if is_tools_rejection(&e) => {
                    // Provider chokes on the tools array: one plain retry,
                    // then whatever comes back is final.
                    eprintln!("[agent] tools rejected, plain retry: {e:?}");
                    let note: String = format!("run: tools call failed, plain retry: {e:?}")
                        .chars()
                        .take(160)
                        .collect();
                    crate::diagnostics::push(note);
                    let plain: ChatResponse = provider.chat(messages.clone(), &opts).await?;
                    return Ok((plain.text, trace, tool_turns));
                }
                Err(e) => return Err(e),
            }
        };
        last_text = resp.text.clone();
        if debug_enabled() {
            eprintln!(
                "[agent:debug] iter={iters} text={:?} calls={:?}",
                truncate_chars(&resp.text, 160),
                resp.tool_calls
                    .iter()
                    .map(|c| format!(
                        "{}:{}",
                        c.function.name,
                        truncate_chars(&c.function.arguments.as_string(), 120)
                    ))
                    .collect::<Vec<_>>(),
            );
        }

        if resp.tool_calls.is_empty() {
            return Ok((resp.text, trace, tool_turns));
        }
        messages.push(assistant_echo(&resp.text, &resp.tool_calls));

        for tc in &resp.tool_calls {
            let args = tc.function.arguments.to_value();
            let action_id =
                sink.action_started(&tc.function.name, &title_for(&tc.function.name, &args));
            let outcome = execute(&tc.function.name, &args, policy).await;
            if let Some(ref id) = action_id {
                sink.action_finished(id, outcome.ok);
            }
            trace.push(TraceStep {
                tool: tc.function.name.clone(),
                args,
                action_id,
                ok: outcome.ok,
                output_preview: preview(&outcome.output),
            });
            let turn = tool_message(&tc.id, &tc.function.name, &outcome);
            messages.push(turn.clone());
            tool_turns.push(turn);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{
        ChatResponse, ProviderError, ToolArgs, ToolCall, ToolChatResponse, ToolFunction,
    };
    use std::sync::Mutex;

    /// Canned provider: first call requests get_system_context, then answers.
    struct Fake {
        calls: Mutex<usize>,
    }

    impl LlmProvider for Fake {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn default_model(&self) -> &str {
            "fake-1"
        }

        async fn chat(
            &self,
            _messages: Vec<ChatMessage>,
            opts: &ChatOptions,
        ) -> Result<ChatResponse, ProviderError> {
            Ok(ChatResponse {
                text: "final".into(),
                model: opts.model.clone(),
            })
        }

        async fn chat_with_tools(
            &self,
            _messages: Vec<ChatMessage>,
            _opts: &ChatOptions,
            _tools: &[serde_json::Value],
        ) -> Result<ToolChatResponse, ProviderError> {
            let mut n = self.calls.lock().unwrap();
            *n += 1;
            if *n == 1 {
                Ok(ToolChatResponse {
                    text: String::new(),
                    tool_calls: vec![ToolCall {
                        id: None,
                        kind: None,
                        function: ToolFunction {
                            name: "get_system_context".into(),
                            arguments: ToolArgs::Obj(serde_json::json!({})),
                        },
                    }],
                })
            } else {
                Ok(ToolChatResponse {
                    text: "done".into(),
                    tool_calls: vec![],
                })
            }
        }

        async fn models(&self) -> Result<Vec<String>, ProviderError> {
            Ok(vec![])
        }
    }

    struct VecSink {
        started: Mutex<Vec<(String, String)>>,
        finished: Mutex<Vec<(String, bool)>>,
    }

    impl ActionSink for VecSink {
        fn action_started(&self, kind: &str, title: &str) -> Option<String> {
            // Read-only tools skip the Action row, like production.
            if kind == "get_system_context" || kind == "list_processes" {
                return None;
            }
            let id = format!("act-{}", self.started.lock().unwrap().len());
            self.started
                .lock()
                .unwrap()
                .push((kind.into(), title.into()));
            Some(id)
        }

        fn action_finished(&self, action_id: &str, ok: bool) {
            self.finished.lock().unwrap().push((action_id.into(), ok));
        }
    }

    #[tokio::test]
    async fn loop_executes_tools_and_returns() {
        let fake = Fake {
            calls: Mutex::new(0),
        };
        let sink = VecSink {
            started: Mutex::new(vec![]),
            finished: Mutex::new(vec![]),
        };
        let policy = Policy { allow_risky: false };
        let (reply, trace, tool_turns) = run_loop(
            &fake,
            "fake-1",
            vec![ChatMessage {
                role: Role::User,
                content: "hi".into(),
                tool_calls: None,
            }],
            "sys",
            &sink,
            &policy,
            5,
        )
        .await
        .unwrap();
        assert_eq!(reply, "done");
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].tool, "get_system_context");
        assert!(trace[0].ok);
        assert!(trace[0].output_preview.contains("\"os\""));
        // The tool turn is returned for persistence (role "tool").
        assert_eq!(tool_turns.len(), 1);
        assert!(matches!(
            tool_turns[0].role,
            crate::ai::provider::Role::Tool
        ));
        assert!(trace[0].output_preview.contains("\"os\""));
        // Read-only: traced, but no Action row.
        assert!(trace[0].action_id.is_none());
        assert!(sink.started.lock().unwrap().is_empty());
    }

    /// Scripted provider without native tools: first reply carries a fenced
    /// call, second is the final summary.
    struct FakeText {
        calls: Mutex<usize>,
    }

    impl LlmProvider for FakeText {
        fn name(&self) -> &'static str {
            "freetext"
        }

        fn default_model(&self) -> &str {
            "free-1"
        }

        fn supports_tools(&self, _model: &str) -> bool {
            false
        }

        async fn chat(
            &self,
            _messages: Vec<ChatMessage>,
            opts: &ChatOptions,
        ) -> Result<ChatResponse, ProviderError> {
            let mut n = self.calls.lock().unwrap();
            *n += 1;
            let text = if *n == 1 {
                "Consulto el contexto.\n```tool\n{\"name\": \"get_system_context\", \"arguments\": {}}\n```"
            } else {
                "listo"
            };
            Ok(ChatResponse {
                text: text.into(),
                model: opts.model.clone(),
            })
        }

        async fn chat_with_tools(
            &self,
            _messages: Vec<ChatMessage>,
            _opts: &ChatOptions,
            _tools: &[serde_json::Value],
        ) -> Result<ToolChatResponse, ProviderError> {
            panic!("text-mode provider must never receive tools");
        }

        async fn models(&self) -> Result<Vec<String>, ProviderError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn text_mode_executes_fenced_calls() {
        let fake = FakeText {
            calls: Mutex::new(0),
        };
        let sink = VecSink {
            started: Mutex::new(vec![]),
            finished: Mutex::new(vec![]),
        };
        let policy = Policy { allow_risky: false };
        let (reply, trace, tool_turns) = run_loop(
            &fake,
            "free-1",
            vec![ChatMessage {
                role: Role::User,
                content: "hi".into(),
                tool_calls: None,
            }],
            "sys",
            &sink,
            &policy,
            5,
        )
        .await
        .unwrap();
        assert_eq!(reply, "listo");
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].tool, "get_system_context");
        assert!(trace[0].ok);
        assert!(trace[0].output_preview.contains("\"os\""));
        assert_eq!(tool_turns.len(), 1);
    }

    #[test]
    fn text_parser_accepts_only_known_tools() {
        let calls = parse_text_calls(
            "abro esto\n```tool\n{\"name\": \"open_app\", \"arguments\": {\"name\": \"x\"}}\n```\ncola",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "open_app");
        // Unknown names, malformed JSON and non-object args are skipped;
        // a bad block never swallows the good one after it.
        let calls = parse_text_calls(
            "```tool\n{\"name\": \"rm_rf\", \"arguments\": {}}\n```\n\
             ```tool\nnot json\n```\n\
             ```tool\n{\"name\": \"press_key\", \"arguments\": \"space\"}\n```\n\
             ```tool\n{\"name\": \"press_key\", \"arguments\": {\"key\": \"space\"}}\n```",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "press_key");
        // Alternate spellings and fence tags parse too.
        let calls = parse_text_calls(
            "miro esto\n```json\n{\"tool\": \"list_processes\", \"args\": {\"limit\": 5}}\n```",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "list_processes");
        // Consumed blocks are cut from user-visible text; other fences stay.
        let (calls, cleaned) = extract_text_calls(
            "Hardware:\n```tool\n{\"name\": \"get_system_context\", \"arguments\": {}}\n```\n\
             y además\n```python\nprint({\"name\": \"x\"})\n```",
        );
        assert_eq!(calls.len(), 1);
        assert!(!cleaned.contains("get_system_context"));
        assert!(cleaned.contains("Hardware:"));
        assert!(cleaned.contains("print("));
        assert!(parse_text_calls("plain answer, no fence").is_empty());
    }

    #[test]
    fn turn_reminder_targets_newest_user_message() {
        let user = || ChatMessage {
            role: Role::User,
            content: "do it".into(),
            tool_calls: None,
        };
        let assistant = || ChatMessage {
            role: Role::Assistant,
            content: "done".into(),
            tool_calls: None,
        };
        let mut messages = vec![user(), assistant(), user()];
        inject_turn_reminder(&mut messages);
        // Only the newest user turn carries it, exactly once.
        assert!(!messages[0].content.contains("THIS turn"));
        assert!(messages[2].content.contains("THIS turn"));
        inject_turn_reminder(&mut messages);
        assert_eq!(
            messages[2]
                .content
                .matches("in THIS turn confirms it")
                .count(),
            1
        );
        // No user message: no-op, never panics.
        let mut messages = vec![assistant()];
        inject_turn_reminder(&mut messages);
        assert_eq!(messages[0].content, "done");
    }
}
