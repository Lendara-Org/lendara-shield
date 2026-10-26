//! Shared helpers for the HTTP tests. Each `tests/*.rs` file is its own
//! crate, so unused helpers in one of them are expected.
#![allow(dead_code)]

use std::{path::Path, sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, HeaderValue, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use shield_api::{
    HttpOptions, build_app, deployment::PoolDeployment, http::InfoResponse, state::AppState,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

pub const WEB_ORIGIN: &str = "http://localhost:3000";
pub const ANCHOR: &str = "lendara-monorepo-fixh.vercel.app";

/// The deployment fixture: a real SPP manifest shape with public testnet ids.
pub fn deployment() -> Result<PoolDeployment, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deployments.json");
    Ok(PoolDeployment::load(&path)?)
}

/// The app with ramp mode enabled against the sandbox anchor.
pub fn app(db: PgPool) -> Result<Router, Box<dyn std::error::Error>> {
    app_with_ramp(db, Some(ANCHOR.to_owned()))
}

pub fn app_with_ramp(
    db: PgPool,
    ramp_anchor_home_domain: Option<String>,
) -> Result<Router, Box<dyn std::error::Error>> {
    let info = InfoResponse::new(&deployment()?, ramp_anchor_home_domain);
    Ok(build_app(
        AppState {
            db,
            info: Arc::new(info),
        },
        HttpOptions {
            cors_allowed_origins: vec![HeaderValue::from_static(WEB_ORIGIN)],
        },
    ))
}

/// A pool that points at a closed port, to simulate a database outage.
pub fn unreachable_pool() -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(300))
        .connect_lazy("postgres://shield:shield@127.0.0.1:1/shield")
}

pub async fn send(
    app: Router,
    request: Request<Body>,
) -> Result<(StatusCode, HeaderMap, Value), Box<dyn std::error::Error>> {
    let response = app.oneshot(request).await?;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await?.to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)?
    };
    Ok((status, headers, body))
}

pub fn get(uri: &str) -> Result<Request<Body>, axum::http::Error> {
    Request::get(uri).body(Body::empty())
}
