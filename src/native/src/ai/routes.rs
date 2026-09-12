//! HTTP handlers for local-model capabilities that need no user:
//! live provider detection. Chat, selection and sessions live in chat/routes
//! because everything persisted is scoped to the authed user.
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use super::provider::{Provider, ProviderError};
use crate::api::AppState;

pub(crate) fn error_response(e: &ProviderError) -> Response {
    let (status_u16, code, message) = e.http_parts();
    let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(serde_json::json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

fn needs_key(id: &str) -> bool {
    id == "opencode"
}

async fn probe(id: &str) -> serde_json::Value {
    match Provider::resolve(id) {
        Ok(p) => match p.models().await {
            Ok(models) => serde_json::json!({
                "id": id, "name": p.name(), "available": true,
                "models": models, "default_model": p.default_model(),
                "needs_key": needs_key(id), "context_window": p.context_window(),
            }),
            Err(_) => serde_json::json!({
                "id": id, "name": p.name(), "available": false,
                "models": [], "default_model": p.default_model(),
                "needs_key": needs_key(id), "context_window": p.context_window(),
            }),
        },
        Err(_) => serde_json::json!({
            "id": id, "name": id, "available": false,
            "models": [], "default_model": "",
            "needs_key": needs_key(id), "context_window": null,
        }),
    }
}

/// Live detection: probes every known provider concurrently.
/// The UI offers only what answers; the rest renders as unavailable.
pub async fn providers(State(_s): State<AppState>) -> impl IntoResponse {
    let (ollama, llamacpp, opencode) =
        tokio::join!(probe("ollama"), probe("llama.cpp"), probe("opencode"));
    (
        StatusCode::OK,
        Json(serde_json::json!({ "providers": [ollama, llamacpp, opencode] })),
    )
        .into_response()
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
