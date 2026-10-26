//! HTTP tests for the router: health, 404, request id and CORS.
//!
//! `#[sqlx::test]` creates an isolated database per test (from
//! `DATABASE_URL`) and applies the migrations before the test runs.

mod common;

use axum::{
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode, header},
};
use common::{TestResult, WEB_ORIGIN, app, get, send, unreachable_pool};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn health_is_ok_when_the_database_answers(db: PgPool) -> TestResult {
    let (status, _, body) = send(app(db)?, get("/health")?).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"status": "ok", "database": "ok"}));
    Ok(())
}

#[tokio::test]
async fn health_is_degraded_when_the_database_is_unreachable() -> TestResult {
    let (status, _, body) = send(app(unreachable_pool()?)?, get("/health")?).await?;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body,
        json!({"status": "degraded", "database": "unreachable"})
    );
    Ok(())
}

#[tokio::test]
async fn unknown_routes_return_a_json_404() -> TestResult {
    let (status, _, body) = send(app(unreachable_pool()?)?, get("/v1/nope")?).await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
    assert!(body["message"].is_string());
    Ok(())
}

#[tokio::test]
async fn generates_a_request_id_when_the_client_sends_none() -> TestResult {
    let (_, headers, _) = send(app(unreachable_pool()?)?, get("/v1/nope")?).await?;

    let id = headers
        .get("x-request-id")
        .ok_or("missing x-request-id")?
        .to_str()?;
    assert_eq!(id.len(), 36, "expected a UUID, got {id}");
    Ok(())
}

#[tokio::test]
async fn propagates_the_client_request_id() -> TestResult {
    let request = Request::get("/v1/nope")
        .header("x-request-id", "client-chosen-id")
        .body(Body::empty())?;
    let (_, headers, _) = send(app(unreachable_pool()?)?, request).await?;

    assert_eq!(
        headers.get("x-request-id"),
        Some(&HeaderValue::from_static("client-chosen-id"))
    );
    Ok(())
}

fn preflight(origin: &str) -> Result<Request<Body>, axum::http::Error> {
    Request::builder()
        .method(Method::OPTIONS)
        .uri("/health")
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .body(Body::empty())
}

#[tokio::test]
async fn cors_allows_configured_origins() -> TestResult {
    let (_, headers, _) = send(app(unreachable_pool()?)?, preflight(WEB_ORIGIN)?).await?;

    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&HeaderValue::from_static(WEB_ORIGIN))
    );
    Ok(())
}

#[tokio::test]
async fn cors_rejects_other_origins() -> TestResult {
    let (_, headers, _) = send(
        app(unreachable_pool()?)?,
        preflight("https://evil.example")?,
    )
    .await?;

    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    Ok(())
}

#[sqlx::test]
async fn migrations_create_the_updated_at_trigger_function(db: PgPool) -> TestResult {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_proc WHERE proname = 'set_updated_at')",
    )
    .fetch_one(&db)
    .await?;

    assert!(exists);
    Ok(())
}
