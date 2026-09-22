//! HTTP handlers for the auth domain. JSON in, JSON out, errors mapped.
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use serde::Deserialize;

use super::service;
use crate::api::{AppError, AppState, AuthedUser};

#[derive(Deserialize)]
pub struct EmailPassword {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshBody {
    pub refresh_token: String,
}

pub async fn register(
    State(s): State<AppState>,
    Json(b): Json<EmailPassword>,
) -> Result<impl IntoResponse, AppError> {
    let u = service::register(&b.email, &b.password, &s).map_err(AppError)?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "user": u }))))
}

pub async fn login(
    State(s): State<AppState>,
    Json(b): Json<EmailPassword>,
) -> Result<impl IntoResponse, AppError> {
    let (u, pair) = service::login(&b.email, &b.password, &s).map_err(AppError)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "user": u, "tokens": pair })),
    ))
}

pub async fn refresh(
    State(s): State<AppState>,
    Json(b): Json<RefreshBody>,
) -> Result<impl IntoResponse, AppError> {
    let (u, pair) = service::refresh(&b.refresh_token, &s).map_err(AppError)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "user": u, "tokens": pair })),
    ))
}

pub async fn logout(
    State(s): State<AppState>,
    Json(b): Json<RefreshBody>,
) -> Result<impl IntoResponse, AppError> {
    service::logout(&b.refresh_token, &s).map_err(AppError)?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

pub async fn me(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
) -> Result<impl IntoResponse, AppError> {
    let u = service::me(&uid, &s).map_err(AppError)?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "user": u }))))
}

pub async fn delete_account(
    State(s): State<AppState>,
    Extension(AuthedUser(uid)): Extension<AuthedUser>,
) -> Result<impl IntoResponse, AppError> {
    service::delete_account(&uid, &s).map_err(AppError)?;
    // Reap the user's pi child (if any): it may hold their key in env.
    s.pi.drop_child(&uid).await;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}
