//! Generic client for OpenAI-compatible `/v1/chat/completions` servers.
//! One code path serves Ollama's OpenAI endpoint and llama.cpp's server;
//! vendor modules only preset URL/model/defaults (see ollama.rs, llamacpp.rs).
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::provider::{ChatMessage, ChatOptions, ChatResponse, ProviderError};

#[derive(Debug, Clone)]
pub struct OpenAiCompatConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Clone)]
pub struct OpenAiCompatClient {
    http: Client,
    config: OpenAiCompatConfig,
}

#[derive(Serialize)]
struct CompatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Deserialize)]
struct CompatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: CompatMessage,
}

#[derive(Deserialize)]
struct CompatMessage {
    content: Option<String>,
}

impl OpenAiCompatClient {
    pub fn new(config: OpenAiCompatConfig) -> Result<Self, ProviderError> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs.max(1)))
            .build()
            .map_err(|e| ProviderError::Misconfigured(format!("http client: {e}")))?;
        Ok(Self { http, config })
    }

    pub fn endpoint(&self) -> String {
        format!(
            "{}/v1/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    /// Short-timeout GET for capability probing (model lists, liveness).
    /// Separate from chat so detection stays snappy when a server is down.
    pub async fn probe_get(&self, path: &str) -> Result<serde_json::Value, ProviderError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| ProviderError::Misconfigured(format!("http client: {e}")))?;
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
        let mut req = client.get(url);
        if let Some(key) = self.config.api_key.as_deref().filter(|k| !k.is_empty()) {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                ProviderError::Unreachable(e.to_string())
            } else {
                ProviderError::BadResponse(format!("request failed: {e}"))
            }
        })?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(ProviderError::Status(status, String::new()));
        }
        resp.json()
            .await
            .map_err(|e| ProviderError::BadResponse(format!("invalid response: {e}")))
    }

    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        opts: &ChatOptions,
    ) -> Result<ChatResponse, ProviderError> {
        let body = CompatRequest {
            model: &opts.model,
            messages: &messages,
            temperature: opts.temperature,
            max_tokens: opts.max_tokens,
            response_format: opts.json_mode.then_some(ResponseFormat {
                kind: "json_object",
            }),
        };
        let mut req = self.http.post(self.endpoint()).json(&body);
        if let Some(key) = self.config.api_key.as_deref().filter(|k| !k.is_empty()) {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                ProviderError::Unreachable(e.to_string())
            } else {
                ProviderError::BadResponse(format!("request failed: {e}"))
            }
        })?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            let snippet: String = resp
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(300)
                .collect();
            return Err(ProviderError::Status(status, snippet));
        }
        let parsed: CompatResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::BadResponse(format!("invalid chat response: {e}")))?;
        match parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
        {
            Some(text) => Ok(ChatResponse {
                text,
                model: opts.model.clone(),
            }),
            None => Err(ProviderError::BadResponse("empty choices".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Json, Router};
    use serde_json::{json, Value};

    async fn stub_server() -> u16 {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|Json(_): Json<Value>| async {
                Json(json!({"choices": [{"message": {"role": "assistant", "content": "hola"}}]}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        port
    }

    fn client_for(port: u16) -> OpenAiCompatClient {
        OpenAiCompatClient::new(OpenAiCompatConfig {
            base_url: format!("http://127.0.0.1:{port}"),
            api_key: None,
            timeout_secs: 5,
        })
        .unwrap()
    }

    fn opts() -> ChatOptions {
        ChatOptions {
            model: "test-model".into(),
            temperature: None,
            max_tokens: None,
            json_mode: true,
        }
    }

    #[tokio::test]
    async fn parses_openai_chat_response() {
        let port = stub_server().await;
        let res = client_for(port)
            .chat(
                vec![ChatMessage {
                    role: super::super::provider::Role::User,
                    content: "hi".into(),
                }],
                &opts(),
            )
            .await
            .unwrap();
        assert_eq!(res.text, "hola");
    }

    #[tokio::test]
    async fn unreachable_maps_to_unreachable() {
        // Port 1 is (practically) never open on loopback.
        let c = OpenAiCompatClient::new(OpenAiCompatConfig {
            base_url: "http://127.0.0.1:1".into(),
            api_key: None,
            timeout_secs: 2,
        })
        .unwrap();
        assert!(matches!(
            c.chat(vec![], &opts()).await,
            Err(ProviderError::Unreachable(_))
        ));
    }
}
