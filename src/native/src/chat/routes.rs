//! Chat persistence handlers. Every query is scoped by the authed user:
//! sessions, messages and actions of other users are invisible (404).
//!
//! Plain chat only stores text. Actions are created exclusively by command
//! execution (the future executor will POST them); the left pane reads them
//! per session, so old sessions show the actions they caused.
use std::sync::MutexGuard;

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use super::model::Selection;
use super::store::ChatStore;
use crate::{
    ai::{
        provider::{ChatMessage as LlmMessage, ChatOptions, Provider, Role},
        routes::error_response,
    },
    api::{AppState, AuthedUser},
};

#[derive(Deserialize)]
pub struct ChatBody {
    pub session_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub message: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Deserialize)]
pub struct CreateSessionBody {
    pub title: Option<String>,
}

#[derive(Deserialize)]
pub struct SelectBody {
    /// "ollama" | "llama.cpp" (aliases: llamacpp, llama-cpp)
    pub provider: String,
    pub model: Option<String>,
}

#[derive(Deserialize)]
pub struct ActionsQuery {
    pub session_id: Option<String>,
}

fn lock_chat(state: &AppState) -> Result<MutexGuard<'_, ChatStore>, Response> {
    state.chat.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
        )
            .into_response()
    })
}

fn non_empty(v: &Option<String>) -> Option<String> {
    v.clone().filter(|s| !s.trim().is_empty())
}

fn title_of(message: &str) -> String {
    let t: String = message
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    let t: String = t.chars().take(48).collect();
    if t.is_empty() {
        "New chat".into()
    } else {
        t
    }
}

/// Current persisted selection (or the default when never chosen).
pub async fn selection(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
) -> impl IntoResponse {
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    match store.get_selection(&uid) {
        Ok(Some(sel)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "provider": sel.provider, "model": sel.model })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::OK,
            Json(serde_json::json!({ "provider": "ollama", "model": null })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": e.to_string() } })),
        )
            .into_response(),
    }
}

/// Validates provider + model against the live server, then persists them.
/// This is the "connect once selected" step.
pub async fn select(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Json(b): Json<SelectBody>,
) -> impl IntoResponse {
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
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    if store
        .upsert_selection(&uid, provider.name(), model.as_deref())
        .is_err()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({ "provider": provider.name(), "model": model })),
    )
        .into_response()
}

pub async fn chat(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Json(b): Json<ChatBody>,
) -> impl IntoResponse {
    let message = b.message.trim().to_string();
    if message.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": { "code": "validation", "message": "message must not be empty", "field": "message" }
            })),
        )
            .into_response();
    }
    let internal = || {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
        )
            .into_response()
    };

    // Resolve provider/model first: explicit > persisted > default.
    let persisted: Option<Selection> = match lock_chat(&s) {
        Ok(store) => store.get_selection(&uid).ok().flatten(),
        Err(r) => return r,
    };
    let (prov_name, model_name) = match resolve_for_chat(&b, persisted.as_ref()) {
        Ok(v) => v,
        Err(r) => return r,
    };

    // Reuse the session (ownership checked) or create it from the first message.
    let session_id: String = {
        let store = match lock_chat(&s) {
            Ok(g) => g,
            Err(r) => return r,
        };
        match &b.session_id {
            Some(id) => match store.get_session(id, &uid) {
                Ok(Some(sess)) => sess.id,
                _ => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({
                            "error": { "code": "not_found", "message": "session not found" }
                        })),
                    )
                        .into_response();
                }
            },
            None => {
                match store.create_session(&uid, &title_of(&message), &prov_name, Some(&model_name))
                {
                    Ok(sess) => sess.id,
                    Err(_) => return internal(),
                }
            }
        }
    };

    // Store the user message and snapshot the last 20 for context.
    // The lock is released before any await.
    let history: Vec<LlmMessage> = {
        let store = match lock_chat(&s) {
            Ok(g) => g,
            Err(r) => return r,
        };
        if store.add_message(&session_id, "user", &message).is_err() {
            return internal();
        }
        store
            .list_messages(&session_id)
            .unwrap_or_default()
            .into_iter()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|m| LlmMessage {
                role: if m.role == "assistant" {
                    Role::Assistant
                } else {
                    Role::User
                },
                content: m.content,
            })
            .collect()
    };

    chat_with_history(&s, &session_id, &prov_name, &model_name, history, &b).await
}

fn resolve_for_chat(
    b: &ChatBody,
    persisted: Option<&super::model::Selection>,
) -> Result<(String, String), Response> {
    let prov_name = non_empty(&b.provider).unwrap_or_else(|| {
        persisted
            .map(|p| p.provider.clone())
            .unwrap_or_else(|| "ollama".into())
    });
    let provider = match Provider::resolve(&prov_name) {
        Ok(p) => p,
        Err(e) => return Err(error_response(&e)),
    };
    let model = non_empty(&b.model)
        .or_else(|| {
            persisted.and_then(|p| {
                if p.provider == provider.name() {
                    p.model.clone()
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| provider.default_model().to_string());
    Ok((provider.name().to_string(), model))
}

async fn chat_with_history(
    s: &AppState,
    session_id: &str,
    prov_name: &str,
    model_name: &str,
    history: Vec<LlmMessage>,
    b: &ChatBody,
) -> Response {
    let provider = match Provider::resolve(prov_name) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };
    let opts = ChatOptions {
        model: model_name.to_string(),
        temperature: b.temperature,
        max_tokens: b.max_tokens,
        json_mode: false,
    };
    match provider.chat(history, &opts).await {
        Ok(r) => {
            if let Ok(store) = s.chat.lock() {
                let _ = store.add_message(session_id, "assistant", &r.text);
                let _ = store.touch_session(session_id, provider.name(), Some(&r.model));
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "reply": r.text, "model": r.model,
                    "provider": provider.name(), "session_id": session_id,
                })),
            )
                .into_response()
        }
        Err(e) => error_response(&e),
    }
}

pub async fn list_sessions(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
) -> impl IntoResponse {
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    match store.list_sessions(&uid) {
        Ok(list) => (StatusCode::OK, Json(serde_json::json!({ "sessions": list }))).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
        )
            .into_response(),
    }
}

pub async fn create_session(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Json(b): Json<CreateSessionBody>,
) -> impl IntoResponse {
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    let title = non_empty(&b.title).unwrap_or_else(|| "New chat".into());
    match store.create_session(&uid, &title, "ollama", None) {
        Ok(sess) => (StatusCode::CREATED, Json(serde_json::json!({ "session": sess }))).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
        )
            .into_response(),
    }
}

pub async fn get_session(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    let session = match store.get_session(&id, &uid) {
        Ok(Some(sess)) => sess,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": { "code": "not_found", "message": "session not found" }
                })),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
            )
                .into_response();
        }
    };
    let messages = store.list_messages(&id).unwrap_or_default();
    let actions = store.list_actions_by_session(&id).unwrap_or_default();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "session": session, "messages": messages, "actions": actions })),
    )
        .into_response()
}

pub async fn delete_session(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    match store.delete_session(&id, &uid) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        _ => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": { "code": "not_found", "message": "session not found" }
            })),
        )
            .into_response(),
    }
}

pub async fn list_actions(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Query(q): Query<ActionsQuery>,
) -> impl IntoResponse {
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    let actions = match &q.session_id {
        Some(sid) => {
            // Ownership check doubles as the 404.
            match store.get_session(sid, &uid) {
                Ok(Some(_)) => store.list_actions_by_session(sid),
                _ => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({
                            "error": { "code": "not_found", "message": "session not found" }
                        })),
                    )
                        .into_response();
                }
            }
        }
        None => store.list_recent_actions(&uid, 20),
    };
    match actions {
        Ok(list) => (StatusCode::OK, Json(serde_json::json!({ "actions": list }))).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateActionBody {
    pub session_id: Option<String>,
    pub kind: String,
    pub title: String,
}

#[derive(Deserialize)]
pub struct UpdateActionBody {
    /// running | done | failed
    pub status: String,
}

/// Records a real command execution. Today only the (future) executor calls
/// this; plain chat never does, so the left pane stays truthful.
pub async fn create_action(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Json(b): Json<CreateActionBody>,
) -> impl IntoResponse {
    let kind = b.kind.trim().to_string();
    let title = b.title.trim().to_string();
    if kind.is_empty() || title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": { "code": "validation", "message": "kind and title are required", "field": "title" }
            })),
        )
            .into_response();
    }
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    if let Some(ref sid) = b.session_id {
        match store.get_session(sid, &uid) {
            Ok(Some(_)) => {}
            _ => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": { "code": "not_found", "message": "session not found" }
                    })),
                )
                    .into_response();
            }
        }
    }
    match store.create_action(b.session_id.as_deref(), &uid, &kind, &title) {
        Ok(action) => {
            (StatusCode::CREATED, Json(serde_json::json!({ "action": action }))).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": { "code": "internal", "message": "internal server error" } })),
        )
            .into_response(),
    }
}

pub async fn update_action(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
    Path(id): Path<String>,
    Json(b): Json<UpdateActionBody>,
) -> impl IntoResponse {
    if !["running", "done", "failed"].contains(&b.status.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": { "code": "validation", "message": "status must be running, done or failed", "field": "status" }
            })),
        )
            .into_response();
    }
    let store = match lock_chat(&s) {
        Ok(g) => g,
        Err(r) => return r,
    };
    match store.set_action_status_owned(&id, &uid, &b.status) {
        Ok(true) => match store.get_action(&id, &uid) {
            Ok(Some(action)) => (
                StatusCode::OK,
                Json(serde_json::json!({ "action": action })),
            )
                .into_response(),
            _ => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        },
        _ => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": { "code": "not_found", "message": "action not found" }
            })),
        )
            .into_response(),
    }
}
