//! Reusable LLM abstraction. Everything downstream (routes, future
//! orchestrator, tests with fakes) programs against [`LlmProvider`] and the
//! [`Provider`] enum — never against a vendor client directly.
use serde::{Deserialize, Serialize};
use std::future::Future;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatOptions {
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub json_mode: bool,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    pub model: String,
}

#[derive(Debug)]
pub enum ProviderError {
    Unreachable(String),
    Status(u16, String),
    BadResponse(String),
    Misconfigured(String),
    UnknownProvider(String),
}

fn trunc(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

impl ProviderError {
    /// (HTTP status, machine code, human message) for the API layer.
    /// Transport-agnostic on purpose: no axum types in the abstraction.
    pub fn http_parts(&self) -> (u16, &'static str, String) {
        match self {
            ProviderError::Unreachable(e) => (
                502,
                "ai_upstream",
                format!("provider unreachable (is it running?): {}", trunc(e, 160)),
            ),
            ProviderError::Status(code, body) if (400..500).contains(code) => (
                400,
                "ai_error",
                format!(
                    "provider rejected the request ({code}): {}",
                    trunc(body, 200)
                ),
            ),
            ProviderError::Status(code, body) => (
                502,
                "ai_upstream",
                format!("provider failed ({code}): {}", trunc(body, 200)),
            ),
            ProviderError::BadResponse(e) => (502, "ai_bad_response", trunc(e, 200)),
            ProviderError::Misconfigured(e) => (500, "misconfigured", trunc(e, 200)),
            ProviderError::UnknownProvider(name) => (
                400,
                "unknown_provider",
                format!("unknown provider: {name} (try ollama, llama.cpp)"),
            ),
        }
    }
}

/// Active provider + model chosen by the user (in-memory, per sidecar run).
#[derive(Debug, Clone)]
pub struct ActiveSelection {
    pub provider: String,
    pub model: Option<String>,
}

impl Default for ActiveSelection {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            model: None,
        }
    }
}

/// The contract every vendor implements. Small on purpose: chat in, text out,
///
/// plus a live model listing so the UI only offers what truly exists.
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn default_model(&self) -> &str;
    fn chat(
        &self,
        messages: Vec<ChatMessage>,
        opts: &ChatOptions,
    ) -> impl Future<Output = Result<ChatResponse, ProviderError>> + Send;
    fn models(&self) -> impl Future<Output = Result<Vec<String>, ProviderError>> + Send;
}

/// Runtime dispatch without trait objects (keeps `impl Future` object-safe).
pub enum Provider {
    Ollama(super::ollama::Ollama),
    LlamaCpp(super::llamacpp::LlamaCpp),
}

impl Provider {
    pub fn resolve(name: &str) -> Result<Self, ProviderError> {
        match name {
            "ollama" => Ok(Self::Ollama(super::ollama::Ollama::new()?)),
            "llama.cpp" | "llamacpp" | "llama-cpp" => {
                Ok(Self::LlamaCpp(super::llamacpp::LlamaCpp::new()?))
            }
            other => Err(ProviderError::UnknownProvider(other.to_string())),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Ollama(p) => p.name(),
            Self::LlamaCpp(p) => p.name(),
        }
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::Ollama(p) => p.default_model(),
            Self::LlamaCpp(p) => p.default_model(),
        }
    }

    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        opts: &ChatOptions,
    ) -> Result<ChatResponse, ProviderError> {
        match self {
            Self::Ollama(p) => p.chat(messages, opts).await,
            Self::LlamaCpp(p) => p.chat(messages, opts).await,
        }
    }

    pub async fn models(&self) -> Result<Vec<String>, ProviderError> {
        match self {
            Self::Ollama(p) => p.models().await,
            Self::LlamaCpp(p) => p.models().await,
        }
    }
}
