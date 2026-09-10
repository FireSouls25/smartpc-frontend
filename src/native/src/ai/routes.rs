//! HTTP handlers for local models: live detection, selection, chat.
//! Thin by design: resolve provider, fill defaults, delegate, map errors.
//! Orchestration (plans, tools) comes later.
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use super::provider::{ActiveSelection, ChatMessage, ChatOptions, Provider, ProviderError};
use crate::api::AppState;

#[derive(Deserialize)]
pub struct ChatBody {
    /// Optional override; falls back to the active selection, then default.
    pub provider: Option<String>,
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub json_mode: Option<bool>,
}

#[derive(Deserialize)]
pub struct SelectBody {
    /// "ollama" | "llama.cpp" (aliases: llamacpp, llama-cpp)
    pub provider: String,
    pub model: Option<String>,
}

fn error_response(e: &ProviderError) -> Response {
    let (status_u16, code, message) = e.http_parts();
    let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(serde_json::json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

fn non_empty(v: &Option<String>) -> Option<String> {
    v.clone().filter(|s| !s.trim().is_empty())
}

async fn probe(id: &str) -> serde_json::Value {
    match Provider::resolve(id) {
        Ok(p) => match p.models().await {
            Ok(models) => serde_json::json!({
                "id": id, "name": p.name(), "available": true,
                "models": models, "default_model": p.default_model(),
            }),
            Err(_) => serde_json::json!({
                "id": id, "name": p.name(), "available": false,
                "models": [], "default_model": p.default_model(),
            }),
        },
        Err(_) => serde_json::json!({
            "id": id, "name": id, "available": false,
            "models": [], "default_model": "",
        }),
    }
}

/// Live detection: probes every known local provider concurrently.
/// The UI offers only what answers; the rest renders as unavailable.
pub async fn providers(State(s): State<AppState>) -> impl IntoResponse {
    let (ollama, llamacpp) = tokio::join!(probe("ollama"), probe("llama.cpp"));
    let active = s
        .active
        .lock()
        .ok()
        .map(|a| serde_json::json!({ "provider": a.provider, "model": a.model }))
        .unwrap_or_else(|| serde_json::json!({ "provider": "ollama", "model": null }));
    (
        StatusCode::OK,
        Json(serde_json::json!({ "providers": [ollama, llamacpp], "active": active })),
    )
        .into_response()
}

/// Validates provider + model against the live server, then stores them
/// as the active selection. This is the "connect once selected" step.
pub async fn select(State(s): State<AppState>, Json(b): Json<SelectBody>) -> impl IntoResponse {
    let provider = match Provider::resolve(&b.provider) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let models = match provider.models().await {
        Ok(m) => m,
        Err(e) => return error_response(&e),
    };
    let model = non_empty(&b.model);
    if let Some(ref m) = model {
        if !models.iter().any(|x| x == m) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {
                        "code": "unknown_model",
                        "message": format!("model not available from {}: {m}", provider.name()),
                    }
                })),
            )
                .into_response();
        }
    }
    if let Ok(mut a) = s.active.lock() {
        a.provider = provider.name().to_string();
        a.model = model.clone();
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({ "provider": provider.name(), "model": model })),
    )
        .into_response()
}

pub async fn chat(State(s): State<AppState>, Json(b): Json<ChatBody>) -> impl IntoResponse {
    if b.messages.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": { "code": "validation", "message": "messages must not be empty", "field": "messages" }
            })),
        )
            .into_response();
    }
    let active: ActiveSelection = s.active.lock().ok().map(|a| a.clone()).unwrap_or_default();
    let prov_name = non_empty(&b.provider).unwrap_or_else(|| active.provider.clone());
    let provider = match Provider::resolve(&prov_name) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    // A stored model only applies to its own provider.
    let model = non_empty(&b.model)
        .or_else(|| {
            if active.provider == provider.name() {
                active.model
            } else {
                None
            }
        })
        .unwrap_or_else(|| provider.default_model().to_string());
    let opts = ChatOptions {
        model,
        temperature: b.temperature,
        max_tokens: b.max_tokens,
        json_mode: b.json_mode.unwrap_or(false),
    };
    match provider.chat(b.messages, &opts).await {
        Ok(r) => (
            StatusCode::OK,
            Json(
                serde_json::json!({ "reply": r.text, "model": r.model, "provider": provider.name() }),
            ),
        )
            .into_response(),
        Err(e) => error_response(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::super::provider::Provider;

    #[test]
    fn resolves_known_providers() {
        assert_eq!(Provider::resolve("ollama").unwrap().name(), "ollama");
        assert_eq!(Provider::resolve("llama.cpp").unwrap().name(), "llama.cpp");
        assert!(Provider::resolve("nope").is_err());
    }
}
