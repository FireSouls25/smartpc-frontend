//! Voice HTTP handlers. Sidecar-gated like provider detection (no user
//! needed for local mic access); transcripts are *not* persisted here — the
//! renderer feeds them into the authed agent endpoint itself.
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;

use super::session::{parse_opts, StartError};
use crate::api::AppState;

fn start_error_response(e: &StartError) -> axum::response::Response {
    let (status_u16, code, message) = e.http_parts();
    let status =
        StatusCode::from_u16(status_u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(serde_json::json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

pub async fn status(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.voice.status())
}

#[derive(Debug, Deserialize)]
pub struct ListenBody {
    pub mode: Option<String>,
    pub wake_word: Option<String>,
    pub lang: Option<String>,
    pub model: Option<String>,
}

/// Starts a session. Blocks on first-use model download (minutes on slow
/// links for larger models) — the client passes a generous timeout.
pub async fn listen(
    State(s): State<AppState>,
    Json(b): Json<ListenBody>,
) -> impl IntoResponse {
    let opts = match parse_opts(b.mode.as_deref(), b.wake_word.as_deref(), b.lang.as_deref(), b.model.as_deref()) {
        Ok(o) => o,
        Err(e) => return start_error_response(&e),
    };
    let voice = s.voice.clone();
    // Sync, blocking, potentially minutes: off the async workers.
    let res = tokio::task::spawn_blocking(move || voice.start(opts)).await;
    match res {
        Ok(Ok(())) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true })),
        )
            .into_response(),
        Ok(Err(e)) => start_error_response(&e),
        Err(_) => start_error_response(&StartError::ModelFailed(
            "voice worker failed".to_string(),
        )),
    }
}

pub async fn stop(State(s): State<AppState>) -> impl IntoResponse {
    s.voice.stop();
    Json(serde_json::json!({ "ok": true }))
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub cursor: Option<u64>,
}

/// Long-poll: returns events after `cursor`, holding up to ~25 s for news.
/// The client loops immediately with the returned cursor.
pub async fn events(
    State(s): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> impl IntoResponse {
    let (events, next) = s.voice.poll(q.cursor.unwrap_or(0)).await;
    Json(serde_json::json!({ "events": events, "next": next }))
}
