//! Ollama provider: OpenAI-compat client preset to Ollama's defaults.
//! Override with OLLAMA_URL / OLLAMA_MODEL / OLLAMA_API_KEY (usually unset).
//! Live models come from Ollama's native `/api/tags`.
use super::{
    openai_compat::{OpenAiCompatClient, OpenAiCompatConfig},
    provider::{
        ChatMessage, ChatOptions, ChatResponse, LlmProvider, ProviderError, ToolChatResponse,
    },
};

pub struct Ollama {
    client: OpenAiCompatClient,
    model: String,
    num_ctx: u32,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

impl Ollama {
    /// Server-side context allocation. Ollama defaults to 4096, which our
    /// agent requests (~4-5K tokens with schemas+history) overflow — hence
    /// lost instructions mid-session. 8192 fits a 2B Q4 + KV in 4GB VRAM.
    pub fn num_ctx_from_env() -> u32 {
        std::env::var("OLLAMA_NUM_CTX")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .map(|n| n.clamp(2048, 65536))
            .unwrap_or(8192)
    }

    pub fn new() -> Result<Self, ProviderError> {
        let num_ctx = Self::num_ctx_from_env();
        Ok(Self {
            client: OpenAiCompatClient::new(OpenAiCompatConfig {
                base_url: env_or("OLLAMA_URL", "http://127.0.0.1:11434"),
                api_key: std::env::var("OLLAMA_API_KEY").ok(),
                timeout_secs: 180,
                options: Some(serde_json::json!({ "num_ctx": num_ctx })),
            })?,
            model: env_or("OLLAMA_MODEL", "llama3.1"),
            num_ctx,
        })
    }

    /// Effective context window (tokens) sent to the server.
    pub fn context_window(&self) -> u32 {
        self.num_ctx
    }

    pub fn parse_tags(v: &serde_json::Value) -> Vec<String> {
        let mut names: Vec<String> = v
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("name")?.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

impl LlmProvider for Ollama {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn default_model(&self) -> &str {
        &self.model
    }

    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        opts: &ChatOptions,
    ) -> Result<ChatResponse, ProviderError> {
        self.client.chat(messages, opts).await
    }

    async fn chat_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        opts: &ChatOptions,
        tools: &[serde_json::Value],
    ) -> Result<ToolChatResponse, ProviderError> {
        self.client.chat_with_tools(messages, opts, tools).await
    }

    async fn models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(Self::parse_tags(&self.client.probe_get("/api/tags").await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ollama_tags() {
        let v = serde_json::json!({
            "models": [
                {"name": "qwen2.5:7b"}, {"name": "llama3.1:8b"}, {"nope": 1}
            ]
        });
        assert_eq!(
            Ollama::parse_tags(&v),
            vec!["llama3.1:8b".to_string(), "qwen2.5:7b".to_string()]
        );
        assert!(Ollama::parse_tags(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn num_ctx_defaults_and_clamps() {
        let prev = std::env::var("OLLAMA_NUM_CTX").ok();
        std::env::remove_var("OLLAMA_NUM_CTX");
        assert_eq!(Ollama::num_ctx_from_env(), 8192);
        std::env::set_var("OLLAMA_NUM_CTX", "16384");
        assert_eq!(Ollama::num_ctx_from_env(), 16384);
        std::env::set_var("OLLAMA_NUM_CTX", "bogus");
        assert_eq!(Ollama::num_ctx_from_env(), 8192);
        std::env::set_var("OLLAMA_NUM_CTX", "512");
        assert_eq!(Ollama::num_ctx_from_env(), 2048);
        match prev {
            Some(v) => std::env::set_var("OLLAMA_NUM_CTX", v),
            None => std::env::remove_var("OLLAMA_NUM_CTX"),
        }
    }
}
