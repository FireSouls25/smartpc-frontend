//! Auth business logic: passwords (argon2id), sessions (JWT + rotation).
//!
//! Same contract as the retired Go service: validation shapes, error cases,
//! rotation with reuse-kills-chain. Well-known crates do the crypto; this
//! file only orchestrates.
use std::sync::MutexGuard;

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::{rngs::OsRng as RandOsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    model::{AuthError, TokenPair, User},
    store::{is_unique_violation, Store},
};
use crate::api::AppState;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iat: usize,
    exp: usize,
}

fn lock_store(state: &AppState) -> Result<MutexGuard<'_, Store>, AuthError> {
    state
        .store
        .lock()
        .map_err(|_| AuthError::Internal("store lock poisoned".into()))
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn normalize_email(raw: &str) -> Result<String, AuthError> {
    let email = raw.trim().to_lowercase();
    if email.is_empty() || email.len() > 254 {
        return Err(AuthError::Validation {
            field: "email",
            message: "email is required",
        });
    }
    if !email_address::EmailAddress::is_valid(&email) {
        return Err(AuthError::Validation {
            field: "email",
            message: "email is not valid",
        });
    }
    Ok(email)
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < 8 {
        return Err(AuthError::Validation {
            field: "password",
            message: "password must be at least 8 characters",
        });
    }
    if password.len() > 128 {
        return Err(AuthError::Validation {
            field: "password",
            message: "password is too long",
        });
    }
    Ok(())
}

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    RandOsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

pub fn register(email: &str, password: &str, state: &AppState) -> Result<User, AuthError> {
    let email = normalize_email(email)?;
    validate_password(password)?;
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AuthError::Internal(e.to_string()))?
        .to_string();
    let store = lock_store(state)?;
    match store.create_user(&random_hex(16), &email, &hash, &now_rfc3339()) {
        Ok(u) => Ok(u),
        Err(e) if is_unique_violation(&e) => Err(AuthError::EmailTaken),
        Err(e) => Err(e.into()),
    }
}

pub fn login(
    email: &str,
    password: &str,
    state: &AppState,
) -> Result<(User, TokenPair), AuthError> {
    let email = normalize_email(email).map_err(|_| AuthError::InvalidCredentials)?;
    let store = lock_store(state)?;
    let (user, hash) = match store.get_user_by_email(&email) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(AuthError::InvalidCredentials),
        Err(e) => return Err(e.into()),
    };
    let parsed = PasswordHash::new(&hash).map_err(|_| AuthError::InvalidCredentials)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AuthError::InvalidCredentials)?;
    Ok((user.clone(), issue_pair(&user.id, state, &store)?))
}

fn issue_pair(
    user_id: &str,
    state: &AppState,
    store: &MutexGuard<'_, Store>,
) -> Result<TokenPair, AuthError> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now,
        exp: now + state.access_ttl_secs as usize,
    };
    let access = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&state.jwt_secret),
    )
    .map_err(|e| AuthError::Internal(e.to_string()))?;
    let refresh = random_hex(32);
    let expires_at = (Utc::now() + chrono::Duration::seconds(state.refresh_ttl_secs))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    store.create_refresh_token(
        &random_hex(16),
        user_id,
        &sha256_hex(&refresh),
        &expires_at,
        &now_rfc3339(),
    )?;
    Ok(TokenPair {
        access_token: access,
        refresh_token: refresh,
        expires_in: state.access_ttl_secs,
    })
}

pub fn refresh(refresh_token: &str, state: &AppState) -> Result<(User, TokenPair), AuthError> {
    let hash = sha256_hex(refresh_token);
    let store = lock_store(state)?;
    let rt = match store.get_refresh_token(&hash) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(AuthError::InvalidToken),
        Err(e) => return Err(e.into()),
    };
    if rt.revoked {
        // A revoked token coming back means possible theft: kill the chain.
        let _ = store.revoke_all_user_tokens(&rt.user_id);
        return Err(AuthError::InvalidToken);
    }
    let expired = chrono::DateTime::parse_from_rfc3339(&rt.expires_at)
        .map(|dt| dt.with_timezone(&Utc) < Utc::now())
        .unwrap_or(true);
    if expired {
        let _ = store.revoke_refresh_token(&hash);
        return Err(AuthError::InvalidToken);
    }
    store.revoke_refresh_token(&hash)?;
    let pair = issue_pair(&rt.user_id, state, &store)?;
    let user = match store.get_user_by_id(&rt.user_id) {
        Ok(u) => u,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(AuthError::InvalidToken),
        Err(e) => return Err(e.into()),
    };
    Ok((user, pair))
}

/// Idempotent: unknown tokens still succeed.
pub fn logout(refresh_token: &str, state: &AppState) -> Result<(), AuthError> {
    let store = lock_store(state)?;
    store.revoke_refresh_token(&sha256_hex(refresh_token))?;
    Ok(())
}

pub fn delete_account(user_id: &str, state: &AppState) -> Result<(), AuthError> {
    let store = lock_store(state)?;
    match store.delete_user(user_id) {
        Ok(()) => Ok(()),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(AuthError::UserNotFound),
        Err(e) => Err(e.into()),
    }
}

pub fn me(user_id: &str, state: &AppState) -> Result<User, AuthError> {
    let store = lock_store(state)?;
    match store.get_user_by_id(user_id) {
        Ok(u) => Ok(u),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(AuthError::UserNotFound),
        Err(e) => Err(e.into()),
    }
}

pub fn verify_access(token: &str, secret: &[u8]) -> Result<String, AuthError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map_err(|_| AuthError::InvalidToken)?;
    if data.claims.sub.is_empty() {
        return Err(AuthError::InvalidToken);
    }
    Ok(data.claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::store::Store;
    use crate::chat::store::ChatStore;
    use std::sync::{Arc, Mutex};

    fn test_state() -> AppState {
        AppState {
            store: Arc::new(Mutex::new(Store::open(":memory:").unwrap())),
            jwt_secret: Arc::new(b"test-secret-that-is-long-enough!!".to_vec()),
            access_ttl_secs: 900,
            refresh_ttl_secs: 3600,
            sidecar_token: Arc::new("test-token-12345678".into()),
            chat: Arc::new(Mutex::new(ChatStore::open(":memory:").unwrap())),
        }
    }

    #[test]
    fn auth_flow() {
        let s = test_state();
        let u = register("Ada@example.com", "correct-horse-1", &s).unwrap();
        assert_eq!(u.email, "ada@example.com");

        assert!(matches!(
            register("ada@example.com", "another-pass-1", &s),
            Err(AuthError::EmailTaken)
        ));
        assert!(matches!(
            register("bad-email", "correct-horse-1", &s),
            Err(AuthError::Validation { .. })
        ));
        assert!(matches!(
            register("bob@example.com", "short", &s),
            Err(AuthError::Validation { .. })
        ));

        assert!(matches!(
            login("ada@example.com", "wrong-password", &s),
            Err(AuthError::InvalidCredentials)
        ));
        assert!(matches!(
            login("nobody@example.com", "whatever-123", &s),
            Err(AuthError::InvalidCredentials)
        ));

        let (user, pair) = login("ada@example.com", "correct-horse-1", &s).unwrap();
        assert!(!pair.access_token.is_empty() && !pair.refresh_token.is_empty());
        assert_eq!(
            verify_access(&pair.access_token, &s.jwt_secret).unwrap(),
            user.id
        );

        let (user2, pair2) = refresh(&pair.refresh_token, &s).unwrap();
        assert_eq!(user2.id, user.id);
        assert!(matches!(
            refresh(&pair.refresh_token, &s),
            Err(AuthError::InvalidToken)
        ));

        logout(&pair2.refresh_token, &s).unwrap();
        assert!(matches!(
            refresh(&pair2.refresh_token, &s),
            Err(AuthError::InvalidToken)
        ));
        logout("unknown-token", &s).unwrap(); // idempotent

        assert_eq!(me(&user.id, &s).unwrap().email, "ada@example.com");
        delete_account(&user.id, &s).unwrap();
        assert!(matches!(me(&user.id, &s), Err(AuthError::UserNotFound)));
    }

    #[test]
    fn expired_refresh_is_rejected() {
        let s = test_state();
        let (user, pair) = {
            let u = register("exp@example.com", "correct-horse-1", &s).unwrap();
            let (_, p) = login("exp@example.com", "correct-horse-1", &s).unwrap();
            (u, p)
        };
        // Backdate the stored expiry straight in SQL.
        {
            let store = s.store.lock().unwrap();
            store
                .conn_for_test()
                .execute(
                    "UPDATE refresh_tokens SET expires_at = '2000-01-01T00:00:00Z'",
                    [],
                )
                .unwrap();
        }
        assert!(matches!(
            refresh(&pair.refresh_token, &s),
            Err(AuthError::InvalidToken)
        ));
        let _ = user;
    }
}
