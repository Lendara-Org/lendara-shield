//! Extractors that report failures in the API's error format instead of
//! axum's plain-text defaults.

use axum::{
    extract::{FromRequest, FromRequestParts, Request, rejection::JsonRejection},
    http::{header, request::Parts},
};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::{error::ApiError, state::AppState};

/// `Json<T>` whose rejection is `400 invalid_request`.
#[derive(Debug)]
pub struct ApiJson<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(req, state)
            .await
            .map(|axum::Json(value)| Self(value))
            .map_err(|rejection: JsonRejection| ApiError::BadRequest {
                code: "invalid_request",
                message: rejection.body_text(),
            })
    }
}

/// Proof that the request carries the admin bearer token.
#[derive(Debug)]
pub struct AdminAuth;

impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = ApiError;

    // Synchronous check: no `async` needed, so return a ready future.
    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(bearer(parts, state.admin_token.as_ref().as_ref()).map(|()| Self))
    }
}

/// Proof that the request carries the auditor bearer token.
#[derive(Debug)]
pub struct AuditorAuth;

impl FromRequestParts<AppState> for AuditorAuth {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(bearer(parts, state.auditor_token.as_ref().as_ref()).map(|()| Self))
    }
}

/// Checks `Authorization: Bearer <expected>`; no token configured rejects all.
fn bearer(parts: &Parts, expected: Option<&SecretString>) -> Result<(), ApiError> {
    let expected = expected.ok_or(ApiError::Unauthorized)?;
    let provided = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    if constant_time_eq(provided.as_bytes(), expected.expose_secret().as_bytes()) {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

/// Compares SHA-256 digests without short-circuiting: both sides always
/// have 32 bytes, so the response time reveals neither how much of the token
/// was right nor how long it is.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let (a, b) = (Sha256::digest(a), Sha256::digest(b));
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn compares_tokens() {
        assert!(constant_time_eq(b"secret-token", b"secret-token"));
        assert!(!constant_time_eq(b"secret-token", b"secret-tokeN"));
        assert!(!constant_time_eq(b"secret-token", b"secret"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn a_prefix_or_an_extension_of_the_token_is_refused() {
        let token = b"0123456789abcdef0123456789abcdef";
        assert!(!constant_time_eq(&token[..31], token));
        assert!(!constant_time_eq(
            b"0123456789abcdef0123456789abcdef0",
            token
        ));
    }
}
