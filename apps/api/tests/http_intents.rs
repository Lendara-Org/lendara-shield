//! `POST /v1/intents` and `GET /v1/intents/{id}` against the contract.

mod common;

use axum::http::StatusCode;
use common::{TestResult, app_with_fakes, app_with_ramp, get, post_json, send};
use serde_json::{Value, json};
use shield_api::testing::{AnchorBehaviour, ChainBehaviour};
use sqlx::PgPool;

const ALICE: &str = "GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC";

fn onboarding() -> Value {
    json!({
        "stellar_account": ALICE,
        "note_public_key": format!("0x{}", "11".repeat(32)),
        "encryption_public_key": format!("0x{}", "22".repeat(32)),
        "asp_leaf": format!("0x{}", "03".repeat(32)),
    })
}

fn intent_request(amount: &str) -> Value {
    json!({"mode": "ramp", "stellar_account": ALICE, "fiat_currency": "CRC", "amount": amount})
}

fn assert_intent_shape(body: &Value) {
    let mut keys: Vec<&str> = body
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "amount_asset",
            "amount_fiat",
            "asset_code",
            "completed_at",
            "created_at",
            "fiat_currency",
            "id",
            "interactive_url",
            "message",
            "mode",
            "status",
            "transactions",
            "updated_at"
        ],
        "response must match the Intent schema exactly"
    );
    assert_eq!(
        body["transactions"].as_object().map(serde_json::Map::len),
        Some(3),
        "the three hash slots are always present"
    );
}

#[sqlx::test]
async fn creates_and_reads_an_intent(db: PgPool) -> TestResult {
    let (app, _, anchor) = app_with_fakes(db)?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &onboarding(), None)?,
    )
    .await?;

    let (status, _, created) = send(
        app.clone(),
        post_json("/v1/intents", &intent_request("25000.00"), None)?,
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_intent_shape(&created);
    assert_eq!(created["mode"], "ramp");
    assert_eq!(created["status"], "awaiting_anchor");
    assert_eq!(created["fiat_currency"], "CRC");
    assert_eq!(created["amount_fiat"], "25000");
    assert_eq!(created["asset_code"], "USDC");
    assert_eq!(created["amount_asset"], Value::Null);
    assert!(
        created["interactive_url"]
            .as_str()
            .is_some_and(|u| u.starts_with("https://"))
    );
    assert_eq!(
        created["transactions"],
        json!({"intake_payment": null, "pool_deposit": null, "pool_transfer": null})
    );
    assert_eq!(anchor.requests().len(), 1);

    let id = created["id"].as_str().ok_or("id")?;
    let (status, _, read) = send(app, get(&format!("/v1/intents/{id}"))?).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, created);
    Ok(())
}

#[sqlx::test]
async fn ramp_mode_disabled_is_a_409(db: PgPool) -> TestResult {
    let app = app_with_ramp(db, None)?;
    let (status, _, err) =
        send(app, post_json("/v1/intents", &intent_request("10"), None)?).await?;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "ramp_mode_disabled");
    Ok(())
}

#[sqlx::test]
async fn unknown_and_not_allowed_users_are_refused(db: PgPool) -> TestResult {
    let (app, chain, anchor) = app_with_fakes(db)?;
    let (status, _, err) = send(
        app.clone(),
        post_json("/v1/intents", &intent_request("10"), None)?,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["error"], "user_not_found");

    // Onboarded but still pending (chain down): not in the allow-list yet.
    chain.set_behaviour(ChainBehaviour::Down);
    send(
        app.clone(),
        post_json("/v1/onboarding", &onboarding(), None)?,
    )
    .await?;
    let (status, _, err) =
        send(app, post_json("/v1/intents", &intent_request("10"), None)?).await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "user_not_allowed");
    assert!(anchor.requests().is_empty());
    Ok(())
}

#[sqlx::test]
async fn anchor_failures_are_a_502_and_leave_an_error_intent(db: PgPool) -> TestResult {
    let (app, _, anchor) = app_with_fakes(db.clone())?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &onboarding(), None)?,
    )
    .await?;
    anchor.set_behaviour(AnchorBehaviour::Down);

    let (status, _, err) =
        send(app, post_json("/v1/intents", &intent_request("10"), None)?).await?;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(err["error"], "anchor_unavailable");

    let stored: String = sqlx::query_scalar("SELECT status FROM intents")
        .fetch_one(&db)
        .await?;
    assert_eq!(stored, "error", "the attempt is kept for audit");
    Ok(())
}

#[sqlx::test]
async fn validates_the_request(db: PgPool) -> TestResult {
    let (app, _, anchor) = app_with_fakes(db)?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &onboarding(), None)?,
    )
    .await?;

    let cases = [
        ("stellar_account", json!("GNOPE"), "invalid_account"),
        ("fiat_currency", json!("crc"), "invalid_currency"),
        ("amount", json!("0"), "invalid_amount"),
        ("amount", json!("-5"), "invalid_amount"),
        ("amount", json!("1.123456789"), "invalid_amount"),
        ("amount", json!(10), "invalid_request"),
        ("mode", json!("direct"), "invalid_request"),
    ];
    for (field, value, code) in cases {
        let mut body = intent_request("10");
        body[field] = value;
        let (status, _, err) = send(app.clone(), post_json("/v1/intents", &body, None)?).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}");
        assert_eq!(err["error"], code, "{field}");
    }

    let mut extra = intent_request("10");
    extra["account"] = json!(ALICE);
    let (status, _, _) = send(app, post_json("/v1/intents", &extra, None)?).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unknown fields are refused"
    );
    assert!(anchor.requests().is_empty());
    Ok(())
}

#[sqlx::test]
async fn reading_validates_the_id(db: PgPool) -> TestResult {
    let (app, _, _) = app_with_fakes(db)?;
    let (status, _, err) = send(app.clone(), get("/v1/intents/not-a-uuid")?).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_request");

    let (status, _, err) = send(
        app,
        get("/v1/intents/0192f5a8-0000-7000-8000-000000000000")?,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["error"], "intent_not_found");
    Ok(())
}
