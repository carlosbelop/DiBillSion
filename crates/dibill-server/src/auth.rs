//! Anonymous device tokens (ADR 0006): 256 random bits, base64url. Only the
//! SHA-256 of a token is stored, so a database leak doesn't leak tokens.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{AppError, AppState};

/// The device making the request, from its bearer token.
#[derive(Debug, Clone, Copy)]
pub struct Device(pub Uuid);

impl FromRequestParts<AppState> for Device {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;
        let client = state.db.get().await?;
        let row = client
            .query_opt(
                "SELECT id FROM devices WHERE token_hash = $1",
                &[&hash(token)],
            )
            .await?;
        row.map(|r| Device(r.get(0))).ok_or(AppError::Unauthorized)
    }
}

/// A fresh random token (also used for invite codes).
pub fn new_token(bytes: usize) -> Result<String, AppError> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| AppError::Internal(format!("no randomness: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

pub fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}
