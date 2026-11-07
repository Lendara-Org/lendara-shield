//! The per-client limit on the public endpoints that cost Shield something.

mod common;

use std::net::SocketAddr;

use axum::{body::Body, extract::ConnectInfo, http::Request, http::StatusCode};
use common::{ALICE, TestResult, get, post_json, rate_limited_app, send, signed_intent};
use serde_json::json;
use sqlx::PgPool;

fn from(
    addr: &str,
    mut request: Request<Body>,
) -> Result<Request<Body>, Box<dyn std::error::Error>> {
    let peer: SocketAddr = addr.parse()?;
    request.extensions_mut().insert(ConnectInfo(peer));
    Ok(request)
}

fn intent() -> serde_json::Value {
    signed_intent(&json!({
        "mode": "ramp", "stellar_account": ALICE, "fiat_currency": "CRC", "amount": "10"
    }))
}

#[sqlx::test]
async fn a_client_over_its_limit_gets_429_until_the_window_resets(db: PgPool) -> TestResult {
    let app = rate_limited_app(db, 2)?;
    for _ in 0..2 {
        let (status, _, _) = send(
            app.clone(),
            from(
                "203.0.113.7:4000",
                post_json("/v1/intents", &intent(), None)?,
            )?,
        )
        .await?;
        assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    }
    let (status, headers, err) = send(
        app.clone(),
        from(
            "203.0.113.7:4001",
            post_json("/v1/intents", &intent(), None)?,
        )?,
    )
    .await?;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(err["error"], "rate_limited");
    let retry: u64 = headers
        .get("retry-after")
        .ok_or("Retry-After")?
        .to_str()?
        .parse()?;
    assert!((1..=60).contains(&retry));

    // Onboarding shares the same budget: it also costs Shield.
    let (status, _, _) = send(
        app.clone(),
        from(
            "203.0.113.7:4002",
            post_json("/v1/onboarding", &json!({}), None)?,
        )?,
    )
    .await?;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    // Another client is not affected, and reads are never limited.
    let (status, _, _) = send(
        app.clone(),
        from(
            "198.51.100.1:4000",
            post_json("/v1/intents", &intent(), None)?,
        )?,
    )
    .await?;
    assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _, _) = send(app, from("203.0.113.7:4003", get("/v1/info")?)?).await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}
