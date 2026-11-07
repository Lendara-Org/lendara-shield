//! Makes every error response use the API's `{ "error", "message" }` body.
//!
//! Handlers already answer through [`ApiError`](crate::error::ApiError), but
//! some errors are produced before or around them: axum's 405 for a wrong
//! method, a path that cannot be decoded (`%FF`), the timeout layers' 408 and
//! the body limit. They come with a plain-text or empty body; this layer
//! rewrites them, keeping the status and headers (such as `Allow`).

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};

use crate::error::ErrorBody;

/// Rewrites an error response whose body is not JSON. Successful responses
/// and errors already in the API's format pass through untouched.
pub async fn ensure_json(response: Response) -> Response {
    let status = response.status();
    if !(status.is_client_error() || status.is_server_error()) || is_json(&response) {
        return response;
    }
    let (code, message) = describe(status);
    let body = serde_json::to_vec(&ErrorBody {
        error: code,
        message: message.to_owned(),
    })
    .unwrap_or_default();
    let (mut parts, _) = response.into_parts();
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Response::from_parts(parts, Body::from(body))
}

fn is_json(response: &Response) -> bool {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"))
}

/// The stable code and a public message for an error status. The original
/// body is dropped: it is framework text, not part of the contract.
fn describe(status: StatusCode) -> (&'static str, &'static str) {
    match status {
        StatusCode::METHOD_NOT_ALLOWED => (
            "method_not_allowed",
            "This method is not allowed on this resource.",
        ),
        StatusCode::REQUEST_TIMEOUT => (
            "request_timeout",
            "The request took too long and was stopped. Try again.",
        ),
        StatusCode::PAYLOAD_TOO_LARGE => ("payload_too_large", "The request body is too large."),
        StatusCode::NOT_FOUND => ("not_found", "The requested resource does not exist."),
        status if status.is_server_error() => ("internal_error", "An unexpected error occurred."),
        _ => ("invalid_request", "The request is not valid."),
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;
    use serde_json::{Value, json};

    use super::*;

    async fn rewrite(
        response: Response,
    ) -> Result<(Response<()>, Value), Box<dyn std::error::Error>> {
        let (parts, body) = ensure_json(response).await.into_parts();
        let bytes = body.collect().await?.to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)?
        };
        Ok((Response::from_parts(parts, ()), value))
    }

    fn plain(status: StatusCode, text: &'static str) -> Result<Response, axum::http::Error> {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::ALLOW, "GET,HEAD")
            .body(Body::from(text))
    }

    #[tokio::test]
    async fn rewrites_framework_errors_and_keeps_their_headers()
    -> Result<(), Box<dyn std::error::Error>> {
        for (status, code) in [
            (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed"),
            (StatusCode::REQUEST_TIMEOUT, "request_timeout"),
            (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
            (StatusCode::BAD_REQUEST, "invalid_request"),
            (StatusCode::NOT_FOUND, "not_found"),
            (StatusCode::BAD_GATEWAY, "internal_error"),
        ] {
            let (parts, body) =
                rewrite(plain(status, "Invalid URL: Invalid UTF-8 in `account`")?).await?;
            assert_eq!(parts.status(), status);
            assert_eq!(parts.headers()[header::CONTENT_TYPE], "application/json");
            assert_eq!(
                parts.headers()[header::ALLOW],
                "GET,HEAD",
                "headers are kept"
            );
            assert_eq!(body["error"], code);
            assert!(
                body["message"]
                    .as_str()
                    .is_some_and(|m| !m.contains("UTF-8")),
                "framework text is dropped"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn an_empty_timeout_body_becomes_json() -> Result<(), Box<dyn std::error::Error>> {
        let empty = Response::builder()
            .status(StatusCode::REQUEST_TIMEOUT)
            .body(Body::empty())?;
        let (_, body) = rewrite(empty).await?;
        assert_eq!(body["error"], "request_timeout");
        Ok(())
    }

    #[tokio::test]
    async fn leaves_successes_and_api_errors_alone() -> Result<(), Box<dyn std::error::Error>> {
        let ok = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from("hello"))?;
        let (parts, _) = ensure_json(ok).await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert!(parts.headers.get(header::CONTENT_TYPE).is_none());

        let api = Response::builder()
            .status(StatusCode::CONFLICT)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"error":"audit_in_progress","message":"x"}"#))?;
        let (_, body) = rewrite(api).await?;
        assert_eq!(body, json!({"error": "audit_in_progress", "message": "x"}));
        Ok(())
    }
}
