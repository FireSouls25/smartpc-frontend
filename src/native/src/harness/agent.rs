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
    ProviderError, Role,
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

fn title_for(tool: &str, args: &serde_json::Value) -> String {
    let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("?");
    match tool {
        "open_app" => format!("Abrir {}", arg("name")),
        "press_key" => format!("Pulsar {}", arg("key")),
        "type_text" => "Escribir texto".to_string(),
        _ => tool.to_string(),
    }
}

fn preview(s: &str) -> String {
    const N: usize = 300;
    if s.len() <= N {
        s.to_string()
    } else {
        format!("{}…", &s[..N])
    }
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

fn is_tools_rejection(e: &ProviderError) -> bool {
    match e {
        ProviderError::Status(code, body) if (400..500).contains(code) => {
            body.to_lowercase().contains("tool")
        }
        _ => false,
    }
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
) -> Result<(String, Vec<TraceStep>), ProviderError> {
    let mut messages = Vec::with_capacity(history.len() + 1);
    messages.push(ChatMessage {
        role: Role::System,
        content: system.to_string(),
        tool_calls: None,
    });
    messages.extend(history);

    let opts = ChatOptions {
        model: model.to_string(),
        temperature: None,
        max_tokens: None,
        json_mode: false,
    };
    let schemas = tools::openai_schemas();
    let mut trace = Vec::new();
    let mut last_text = String::new();
    let mut iters = 0usize;

    loop {
        if iters >= max_iters {
            if last_text.trim().is_empty() {
                last_text = "Alcancé el límite de pasos con trabajo pendiente.".to_string();
            }
            return Ok((last_text, trace));
        }
        iters += 1;

        let resp = match provider
            .chat_with_tools(messages.clone(), &opts, &schemas)
            .await
        {
            Ok(r) => r,
            Err(e) if is_tools_rejection(&e) => {
                // Provider chokes on the tools array: one plain retry,
                // then whatever comes back is final.
                let plain: ChatResponse = provider.chat(messages.clone(), &opts).await?;
                return Ok((plain.text, trace));
            }
            Err(e) => return Err(e),
        };
        last_text = resp.text.clone();

        if resp.tool_calls.is_empty() {
            return Ok((resp.text, trace));
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
            messages.push(tool_message(&tc.id, &tc.function.name, &outcome));
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
        let (reply, trace) = run_loop(
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
        // Read-only: traced, but no Action row.
        assert!(trace[0].action_id.is_none());
        assert!(sink.started.lock().unwrap().is_empty());
    }
}
