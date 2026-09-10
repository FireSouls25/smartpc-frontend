//! Auth domain types and errors.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64, // seconds until the access token expires
}

#[derive(Debug, Clone)]
pub struct RefreshToken {
    pub user_id: String,
    pub expires_at: String,
    pub revoked: bool,
}

#[derive(Debug)]
pub enum AuthError {
    EmailTaken,
    InvalidCredentials,
    InvalidToken,
    UserNotFound,
    Validation {
        field: &'static str,
        message: &'static str,
    },
    Internal(String),
}

impl From<rusqlite::Error> for AuthError {
    fn from(e: rusqlite::Error) -> Self {
        AuthError::Internal(e.to_string())
    }
}
