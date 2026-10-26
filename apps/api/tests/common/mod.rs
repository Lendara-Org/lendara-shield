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
    audit::AuditService,
    build_app,
    deployment::PoolDeployment,
    http::{InfoResponse, rate_limit::RateLimit},
    intent::service::IntentService,
    ports::{AnchorClient, Explorer, PoolAuditor, SystemClock},
    state::AppState,
    testing::{FakeAnchor, FakeAspContract, FakeAuditor, FakeExplorer},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The app plus the fakes behind it, for tests to script and inspect.
pub type Fakes = (Router, Arc<FakeAspContract>, Arc<FakeAnchor>);

pub const WEB_ORIGIN: &str = "http://localhost:3000";
pub const ANCHOR: &str = "lendara-monorepo-fixh.vercel.app";
pub const ADMIN_TOKEN: &str = "test-admin-token-0123456789abcdef0123";
pub const AUDITOR_TOKEN: &str = "test-auditor-token-0123456789abcdef01";

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
    let audit = AuditService::new(db.clone(), None, None);
    app_with(db, anchor, Arc::new(FakeAspContract::default()), audit)
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
    let router = app_with(
        db.clone(),
        Some(anchor.clone()),
        chain.clone(),
        AuditService::new(db, None, None),
    )?;
    Ok((router, chain, anchor))
}

/// The app with a fake auditor (`None`: not configured) and a fake explorer.
pub fn audit_app(
    db: PgPool,
    auditor: Option<Arc<FakeAuditor>>,
    explorer: Arc<FakeExplorer>,
) -> Result<Router, Box<dyn std::error::Error>> {
    let audit = AuditService::new(
        db.clone(),
        auditor.map(|a| a as Arc<dyn PoolAuditor>),
        Some(explorer as Arc<dyn Explorer>),
    );
    app_with(db, None, Arc::new(FakeAspContract::default()), audit)
}

/// The app with ramp mode and a per-client limit of `requests` a minute on
/// onboarding and intent creation.
pub fn rate_limited_app(db: PgPool, requests: u32) -> Result<Router, Box<dyn std::error::Error>> {
    app_with_options(
        db.clone(),
        Some(Arc::new(FakeAnchor::default())),
        Arc::new(FakeAspContract::default()),
        AuditService::new(db, None, None),
        Some(RateLimit {
            requests,
            window: Duration::from_secs(60),
        }),
    )
}

fn app_with(
    db: PgPool,
    anchor: Option<Arc<FakeAnchor>>,
    chain: Arc<FakeAspContract>,
    audit: AuditService,
) -> Result<Router, Box<dyn std::error::Error>> {
    app_with_options(db, anchor, chain, audit, None)
}

fn app_with_options(
    db: PgPool,
    anchor: Option<Arc<FakeAnchor>>,
    chain: Arc<FakeAspContract>,
    audit: AuditService,
    rate_limit: Option<RateLimit>,
) -> Result<Router, Box<dyn std::error::Error>> {
    let info = InfoResponse::new(&deployment()?, anchor.as_ref().map(|_| ANCHOR.to_owned()));
    let asp = AspService::new(db.clone(), chain, Arc::new(SystemClock))
        // Budget for a whole request: room for several steps' database work.
        .with_timing(Duration::from_secs(1), Duration::from_millis(5));
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
            audit,
            admin_token: Arc::new(Some(SecretString::from(ADMIN_TOKEN))),
            auditor_token: Arc::new(Some(SecretString::from(AUDITOR_TOKEN))),
            clock: Arc::new(SystemClock),
        },
        HttpOptions {
            cors_allowed_origins: vec![HeaderValue::from_static(WEB_ORIGIN)],
            rate_limit,
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

/// Test accounts with known seeds, so tests can sign as them (SEP-53).
pub const ALICE: &str = "GDVEU3DD4KOFECV66VIHWEZOYX4ZKR3WV27L464SIIPOU2IUI3JCZA57";
pub const BOB: &str = "GAJZR5RMNUNEK7CRXJVEWXZ5XUXWT7FJGILCDDOITF7EC26RPWJ4UVOE";
const SEEDS: [(&str, u8); 2] = [(ALICE, 7), (BOB, 8)];

/// `body` (an onboarding request) with the signature its account would
/// make. Accounts without a known seed get a well-formed but wrong one.
pub fn signed(body: &Value) -> Value {
    let field = |name: &str| body[name].as_str().unwrap_or_default().to_ascii_lowercase();
    let account = body["stellar_account"].as_str().unwrap_or_default();
    let message = format!(
        "Lendara Shield onboarding\nnetwork: testnet\naccount: {account}\nnote_public_key: {}\n\
         encryption_public_key: {}\nasp_leaf: {}",
        field("note_public_key"),
        field("encryption_public_key"),
        field("asp_leaf"),
    );
    let mut body = body.clone();
    body["signature"] = Value::String(sep53(account, &message));
    body
}

/// `body` (an intent request) with a fresh request id, `issued_at` now and
/// the signature its account would make.
pub fn signed_intent(body: &Value) -> Value {
    signed_intent_with(
        body,
        &uuid::Uuid::now_v7().to_string(),
        chrono::Utc::now().timestamp(),
    )
}

/// Like [`signed_intent`], with a chosen request id and `issued_at`.
pub fn signed_intent_with(body: &Value, request_id: &str, issued_at: i64) -> Value {
    let field = |name: &str| body[name].as_str().unwrap_or_default().to_owned();
    let account = field("stellar_account");
    let message = format!(
        "Lendara Shield intent\nnetwork: testnet\naccount: {account}\nfiat_currency: {}\n\
         amount: {}\nrequest_id: {request_id}\nissued_at: {issued_at}",
        field("fiat_currency"),
        field("amount"),
    );
    let mut body = body.clone();
    body["request_id"] = Value::String(request_id.to_owned());
    body["issued_at"] = Value::from(issued_at);
    body["signature"] = Value::String(sep53(&account, &message));
    body
}

/// SEP-53 signature of `message` by `account` (a known test seed, or a
/// well-formed but wrong key for any other account).
fn sep53(account: &str, message: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};

    let seed = SEEDS
        .iter()
        .find(|(a, _)| *a == account)
        .map_or(99, |(_, s)| *s);
    let digest = Sha256::digest(format!("Stellar Signed Message:\n{message}").as_bytes());
    STANDARD.encode(SigningKey::from_bytes(&[seed; 32]).sign(&digest).to_bytes())
}
