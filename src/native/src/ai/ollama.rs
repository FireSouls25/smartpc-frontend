//! Ollama provider: OpenAI-compat client preset to Ollama's defaults.
//! Override with OLLAMA_URL / OLLAMA_MODEL / OLLAMA_API_KEY (usually unset).
//! Live models come from Ollama's native `/api/tags`.
use super::{
    openai_compat::{OpenAiCompatClient, OpenAiCompatConfig},
    provider::{ChatMessage, ChatOptions, ChatResponse, LlmProvider, ProviderError},
};

pub struct Ollama {
    client: OpenAiCompatClient,
    model: String,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

impl Ollama {
    pub fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            client: OpenAiCompatClient::new(OpenAiCompatConfig {
                base_url: env_or("OLLAMA_URL", "http://127.0.0.1:11434"),
                api_key: std::env::var("OLLAMA_API_KEY").ok(),
                timeout_secs: 180,
            })?,
            model: env_or("OLLAMA_MODEL", "llama3.1"),
        })
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
}
