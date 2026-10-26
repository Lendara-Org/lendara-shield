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
use secrecy::SecretString;
use serde_json::Value;
use shield_api::{
    HttpOptions,
    asp::service::AspService,
    build_app,
    deployment::PoolDeployment,
    http::InfoResponse,
    intent::service::IntentService,
    ports::{AnchorClient, SystemClock},
    state::AppState,
    testing::{FakeAnchor, FakeAspContract},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The app plus the fakes behind it, for tests to script and inspect.
pub type Fakes = (Router, Arc<FakeAspContract>, Arc<FakeAnchor>);

pub const WEB_ORIGIN: &str = "http://localhost:3000";
pub const ANCHOR: &str = "lendara-monorepo-fixh.vercel.app";
pub const ADMIN_TOKEN: &str = "test-admin-token-0123456789abcdef0123";

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
    let anchor = ramp_anchor_home_domain.map(|_| Arc::new(FakeAnchor::default()));
    app_with(db, anchor, Arc::new(FakeAspContract::default()))
}

/// The app wired to a fake chain the test can script and inspect.
pub fn app_with_chain(
    db: PgPool,
) -> Result<(Router, Arc<FakeAspContract>), Box<dyn std::error::Error>> {
    let (router, chain, _) = app_with_fakes(db)?;
    Ok((router, chain))
}

/// The app wired to a fake chain and a fake anchor, both inspectable.
pub fn app_with_fakes(db: PgPool) -> Result<Fakes, Box<dyn std::error::Error>> {
    let chain = Arc::new(FakeAspContract::default());
    let anchor = Arc::new(FakeAnchor::default());
    let router = app_with(db, Some(anchor.clone()), chain.clone())?;
    Ok((router, chain, anchor))
}

fn app_with(
    db: PgPool,
    anchor: Option<Arc<FakeAnchor>>,
    chain: Arc<FakeAspContract>,
) -> Result<Router, Box<dyn std::error::Error>> {
    let info = InfoResponse::new(&deployment()?, anchor.as_ref().map(|_| ANCHOR.to_owned()));
    let asp = AspService::new(db.clone(), chain, Arc::new(SystemClock))
        .with_timing(Duration::from_millis(60), Duration::from_millis(5));
    let intents = IntentService::new(
        db.clone(),
        anchor.map(|a| a as Arc<dyn AnchorClient>),
        "USDC".into(),
    );
    Ok(build_app(
        AppState {
            db,
            info: Arc::new(info),
            asp,
            intents,
            admin_token: Arc::new(Some(SecretString::from(ADMIN_TOKEN))),
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

pub fn post_json(
    uri: &str,
    body: &Value,
    bearer: Option<&str>,
) -> Result<Request<Body>, axum::http::Error> {
    let mut request = Request::post(uri).header("content-type", "application/json");
    if let Some(token) = bearer {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    request.body(Body::from(body.to_string()))
}
