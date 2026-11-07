//! Onboarding, user status and ASP admin endpoints, against the contract.

mod common;

use axum::http::StatusCode;
use common::{ADMIN_TOKEN, ALICE, BOB, TestResult, app_with_chain, get, post_json, send, signed};
use serde_json::{Value, json};
use shield_api::testing::ChainBehaviour;
use sqlx::PgPool;

const NOTE_KEY: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
const ENC_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const LEAF: &str = "0x0333333333333333333333333333333333333333333333333333333333333333";

fn onboarding(scenario: Option<&str>) -> Value {
    let mut body = json!({
        "stellar_account": ALICE,
        "note_public_key": NOTE_KEY,
        "encryption_public_key": ENC_KEY,
        "asp_leaf": LEAF,
    });
    if let Some(scenario) = scenario {
        body["scenario"] = json!(scenario);
    }
    body
}

fn assert_shield_user(body: &Value) {
    let keys: Vec<&str> = body
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    assert_eq!(
        keys,
        [
            "asp_status",
            "created_at",
            "kyc_status",
            "last_asp_tx_hash",
            "stellar_account",
            "updated_at"
        ],
        "response must match the ShieldUser schema exactly (no keys leaked)"
    );
}

#[sqlx::test]
async fn onboarding_creates_an_allowed_user(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    let (status, _, body) = send(
        app,
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_shield_user(&body);
    assert_eq!(body["stellar_account"], ALICE);
    assert_eq!(body["kyc_status"], "approved");
    assert_eq!(body["asp_status"], "allowed");
    assert!(
        body["last_asp_tx_hash"]
            .as_str()
            .is_some_and(|h| h.len() == 64)
    );
    Ok(())
}

#[sqlx::test]
async fn replaying_onboarding_returns_200(db: PgPool) -> TestResult {
    let (app, chain) = app_with_chain(db)?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;
    let (status, _, body) = send(
        app,
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["asp_status"], "allowed");
    assert_eq!(chain.submits(), 1);
    Ok(())
}

#[sqlx::test]
async fn rejected_and_blocked_scenarios(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    let (_, _, rejected) = send(
        app.clone(),
        post_json(
            "/v1/onboarding",
            &signed(&onboarding(Some("rejected"))),
            None,
        )?,
    )
    .await?;
    assert_eq!(rejected["kyc_status"], "rejected");
    assert_eq!(rejected["asp_status"], "not_allowed");
    assert_eq!(rejected["last_asp_tx_hash"], Value::Null);

    let mut bob = onboarding(Some("blocked"));
    bob["stellar_account"] = json!(BOB);
    bob["note_public_key"] = json!(NOTE_KEY.replace('1', "2"));
    bob["asp_leaf"] = json!(LEAF.replace('3', "5"));
    let (_, _, blocked) = send(app, post_json("/v1/onboarding", &signed(&bob), None)?).await?;
    assert_eq!(blocked["kyc_status"], "approved");
    assert_eq!(blocked["asp_status"], "blocked");
    Ok(())
}

#[sqlx::test]
async fn chain_down_returns_a_pending_user(db: PgPool) -> TestResult {
    let (app, chain) = app_with_chain(db)?;
    chain.set_behaviour(ChainBehaviour::Down);
    let (status, _, body) = send(
        app,
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["asp_status"], "pending");
    Ok(())
}

#[sqlx::test]
async fn onboarding_validates_every_field(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    let cases = [
        ("stellar_account", json!("GNOTANACCOUNT"), "invalid_account"),
        ("note_public_key", json!("0x1234"), "invalid_key"),
        ("encryption_public_key", json!("not hex"), "invalid_key"),
        ("asp_leaf", json!(LEAF.to_uppercase()), "invalid_key"),
        ("scenario", json!("maybe"), "invalid_request"),
        // Above the BN254 modulus: big-endian leaf / little-endian note key.
        (
            "asp_leaf",
            json!(format!("0x{}", "f".repeat(64))),
            "invalid_key",
        ),
        (
            "note_public_key",
            json!(format!("0x{}", "f".repeat(64))),
            "invalid_key",
        ),
    ];
    for (field, value, code) in cases {
        let mut body = onboarding(None);
        body[field] = value;
        let (status, _, err) = send(
            app.clone(),
            post_json("/v1/onboarding", &signed(&body), None)?,
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}");
        assert_eq!(err["error"], code, "{field}");
    }
    Ok(())
}

#[sqlx::test]
async fn onboarding_refuses_unknown_fields_such_as_private_keys(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    let mut body = onboarding(None);
    body["note_private_key"] = json!(NOTE_KEY);
    let (status, _, err) = send(app, post_json("/v1/onboarding", &signed(&body), None)?).await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_request");
    Ok(())
}

#[sqlx::test]
async fn onboarding_reports_conflicts(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;

    let mut other_keys = onboarding(None);
    other_keys["asp_leaf"] = json!(LEAF.replace('3', "6"));
    let (status, _, err) = send(
        app.clone(),
        post_json("/v1/onboarding", &signed(&other_keys), None)?,
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "account_already_onboarded");

    let mut other_account = onboarding(None);
    other_account["stellar_account"] = json!(BOB);
    let (status, _, err) = send(
        app,
        post_json("/v1/onboarding", &signed(&other_account), None)?,
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["error"], "pool_key_already_registered");
    Ok(())
}

#[sqlx::test]
async fn malformed_json_is_a_json_400(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    let request = axum::http::Request::post("/v1/onboarding")
        .header("content-type", "application/json")
        .body(axum::body::Body::from("{not json"))?;
    let (status, _, err) = send(app, request).await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_request");
    Ok(())
}

#[sqlx::test]
async fn get_user_returns_the_status(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;
    let (status, _, body) = send(app.clone(), get(&format!("/v1/users/{ALICE}"))?).await?;
    assert_eq!(status, StatusCode::OK);
    assert_shield_user(&body);
    assert_eq!(body["asp_status"], "allowed");

    let (status, _, err) = send(
        app.clone(),
        get("/v1/users/GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY")?,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["error"], "user_not_found");

    let (status, _, err) = send(app, get("/v1/users/nope")?).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_account");
    Ok(())
}

#[sqlx::test]
async fn admin_endpoints_require_the_bearer_token(db: PgPool) -> TestResult {
    let (app, chain) = app_with_chain(db)?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;
    let body = json!({"stellar_account": ALICE});

    for token in [
        None,
        Some("wrong-token"),
        Some(&ADMIN_TOKEN[..ADMIN_TOKEN.len() - 1]),
    ] {
        let (status, _, err) = send(app.clone(), post_json("/v1/asp/block", &body, token)?).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
        assert_eq!(err["error"], "unauthorized");
    }
    assert_eq!(chain.submits(), 1, "no write without the token");
    Ok(())
}

#[sqlx::test]
async fn admin_blocks_and_unblocks(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    send(
        app.clone(),
        post_json("/v1/onboarding", &signed(&onboarding(None)), None)?,
    )
    .await?;
    let body = json!({"stellar_account": ALICE});

    let (status, _, user) = send(
        app.clone(),
        post_json("/v1/asp/block", &body, Some(ADMIN_TOKEN))?,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["asp_status"], "blocked");

    let (_, _, err) = send(
        app.clone(),
        post_json("/v1/asp/block", &body, Some(ADMIN_TOKEN))?,
    )
    .await?;
    assert_eq!(err["error"], "asp_transition_not_allowed");

    let (status, _, user) =
        send(app, post_json("/v1/asp/unblock", &body, Some(ADMIN_TOKEN))?).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["asp_status"], "allowed");
    Ok(())
}

#[sqlx::test]
async fn admin_actions_on_unknown_users_are_404(db: PgPool) -> TestResult {
    let (app, _) = app_with_chain(db)?;
    let (status, _, err) = send(
        app,
        post_json(
            "/v1/asp/unblock",
            &json!({"stellar_account": ALICE}),
            Some(ADMIN_TOKEN),
        )?,
    )
    .await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["error"], "user_not_found");
    Ok(())
}

#[sqlx::test]
async fn onboarding_needs_the_accounts_signature(db: PgPool) -> TestResult {
    let (app, chain) = app_with_chain(db)?;

    // No signature at all.
    let (status, _, err) = send(
        app.clone(),
        post_json("/v1/onboarding", &onboarding(None), None)?,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_request");

    // Bob tries to claim Alice's account with his own keys: signed by Bob.
    let mut squat = signed(&json!({
        "stellar_account": BOB,
        "note_public_key": NOTE_KEY,
        "encryption_public_key": ENC_KEY,
        "asp_leaf": LEAF,
    }));
    squat["stellar_account"] = json!(ALICE);
    let (status, _, err) = send(app.clone(), post_json("/v1/onboarding", &squat, None)?).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_signature");

    // Alice's signature cannot be reused for other keys.
    let mut swapped = signed(&onboarding(None));
    swapped["note_public_key"] = json!(NOTE_KEY.replace('1', "2"));
    let (status, _, err) = send(app.clone(), post_json("/v1/onboarding", &swapped, None)?).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_signature");

    let (status, _, _) = send(app, get(&format!("/v1/users/{ALICE}"))?).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing was registered");
    assert_eq!(chain.submits(), 0, "and nothing reached the chain");
    Ok(())
}
