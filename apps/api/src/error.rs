//! The single error type returned by HTTP handlers.
//!
//! Every error response has the body `{ "error": "<code>", "message": "..." }`
//! where `code` is a stable `snake_case` identifier documented in
//! `openapi.yaml`. Internal details are logged, never sent to the client.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// An error that can be turned into an HTTP response.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// The route or resource does not exist.
    #[error("not found")]
    NotFound,
    /// A specific resource does not exist (`user_not_found`, ...).
    #[error("{message}")]
    Missing { code: &'static str, message: String },
    /// The request is malformed or fails validation.
    #[error("{message}")]
    BadRequest { code: &'static str, message: String },
    /// Missing or wrong credentials.
    #[error("unauthorized")]
    Unauthorized,
    /// The request conflicts with the current state.
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    /// A dependency (database, Stellar RPC, anchor) is unavailable.
    #[error("service unavailable: {0}")]
    Unavailable(String),
    /// Anything unexpected. The source is logged and hidden from the client.
    #[error("internal error: {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Wire format of every error response.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: &'static str,
    pub message: String,
}

impl ApiError {
    /// Stable machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Missing { code, .. }
            | Self::BadRequest { code, .. }
            | Self::Conflict { code, .. } => code,
            Self::Unauthorized => "unauthorized",
            Self::Unavailable(_) => "service_unavailable",
            Self::Internal(_) => "internal_error",
        }
    }

    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound | Self::Missing { .. } => StatusCode::NOT_FOUND,
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Message safe to show to clients.
    fn public_message(&self) -> String {
        match self {
            Self::NotFound => "The requested resource does not exist.".to_owned(),
            Self::Missing { message, .. }
            | Self::BadRequest { message, .. }
            | Self::Conflict { message, .. } => message.clone(),
            Self::Unauthorized => "Missing or invalid bearer token.".to_owned(),
            Self::Unavailable(_) => "A dependency is temporarily unavailable.".to_owned(),
            Self::Internal(_) => "An unexpected error occurred.".to_owned(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::Internal(Box::new(err))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::error!(error = %self, code = self.code(), "request failed");
        }
        let body = ErrorBody {
            error: self.code(),
            message: self.public_message(),
        };
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;
    use serde_json::{Value, json};

    use super::*;

    async fn render(err: ApiError) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
        let response = err.into_response();
        let status = response.status();
        let bytes = response.into_body().collect().await?.to_bytes();
        Ok((status, serde_json::from_slice(&bytes)?))
    }

    #[tokio::test]
    async fn not_found_maps_to_404() -> Result<(), Box<dyn std::error::Error>> {
        let (status, body) = render(ApiError::NotFound).await?;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not_found");
        Ok(())
    }

    #[tokio::test]
    async fn bad_request_keeps_its_code_and_message() -> Result<(), Box<dyn std::error::Error>> {
        let err = ApiError::BadRequest {
            code: "invalid_amount",
            message: "amount must be positive".to_owned(),
        };
        let (status, body) = render(err).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            json!({"error": "invalid_amount", "message": "amount must be positive"})
        );
        Ok(())
    }

    #[tokio::test]
    async fn specific_errors_keep_their_codes() -> Result<(), Box<dyn std::error::Error>> {
        let cases = [
            (
                ApiError::Missing {
                    code: "user_not_found",
                    message: "no".into(),
                },
                StatusCode::NOT_FOUND,
                "user_not_found",
            ),
            (
                ApiError::Conflict {
                    code: "user_not_allowed",
                    message: "no".into(),
                },
                StatusCode::CONFLICT,
                "user_not_allowed",
            ),
            (
                ApiError::Unauthorized,
                StatusCode::UNAUTHORIZED,
                "unauthorized",
            ),
        ];
        for (err, expected_status, expected_code) in cases {
            let (status, body) = render(err).await?;
            assert_eq!(status, expected_status);
            assert_eq!(body["error"], expected_code);
            assert!(body["message"].is_string());
        }
        Ok(())
    }

    #[tokio::test]
    async fn unavailable_hides_the_cause() -> Result<(), Box<dyn std::error::Error>> {
        let (status, body) = render(ApiError::Unavailable("db at 10.0.0.5 down".into())).await?;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "service_unavailable");
        assert!(!body.to_string().contains("10.0.0.5"));
        Ok(())
    }

    #[tokio::test]
    async fn internal_hides_the_source() -> Result<(), Box<dyn std::error::Error>> {
        let err = ApiError::from(sqlx::Error::Protocol("secret detail".into()));
        let (status, body) = render(err).await?;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"], "internal_error");
        assert!(!body.to_string().contains("secret detail"));
        Ok(())
    }
}
