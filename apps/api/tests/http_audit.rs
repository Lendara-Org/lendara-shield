//! `/v1/audit/*`: the auditor's decrypted view (behind `AUDITOR_TOKEN`) and
//! the public view, with a fake auditor and explorer over a real Postgres.

mod common;

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use common::{AUDITOR_TOKEN, TestResult, audit_app, get, send};
use serde_json::json;
use shield_api::{
    asp::{AspStatus, KycScenario, KycStatus},
    domain::{AccountId, PoolField, TxHash},
    ports::{AuditedNote, AuditedTx, PublicTx},
    store::users::{self, NewUser},
    testing::{FakeAuditor, FakeExplorer},
};
use sqlx::PgPool;

const BOB: &str = "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY";
const INTAKE: &str = "GCRN35RTRRZAIFFEMOTZFV7PNPMFOTDNXQKLMOQFSMOBU3LAON3A6NJC";
/// Bob's key as registered, and as the SDK's audit prints it (reversed).
const BOB_KEY: &str = "0xa2a6d71c547705e9710bf15f7853704dca6ebad46d2e6357eafc99fe332f3824";
const BOB_AUDITED: &str = "0x24382f33fe99fcea57632e6dd4ba6eca4d7053785ff10b71e90577541cd7a6a2";
const INTAKE_KEY: &str = "0x5da87d0000000000000000000000000000000000000000000000000000006d09";

fn field(raw: &str) -> PoolField {
    PoolField::parse(raw).unwrap_or_else(|_| PoolField::from_bytes([0; 32]))
}

fn audited(key: &str) -> PoolField {
    PoolField::from_bytes(field(key).field_le())
}

fn note(owner: PoolField, stroops: i64) -> AuditedNote {
    AuditedNote { owner, stroops }
}

/// The ramp deposit and private transfer of bob, plus a withdraw by a key
/// Shield does not know. Newest first.
fn history() -> Vec<AuditedTx> {
    let stranger = field(&format!("0x{}", "77".repeat(32)));
    vec![
        AuditedTx {
            ledger: 30,
            inputs: vec![note(stranger.clone(), 30_000_000)],
            outputs: vec![note(stranger, 10_000_000)],
        },
        AuditedTx {
            ledger: 20,
            inputs: vec![note(audited(INTAKE_KEY), 93_039_215)],
            outputs: vec![note(field(BOB_AUDITED), 93_039_215)],
        },
        AuditedTx {
            ledger: 10,
            inputs: vec![],
            outputs: vec![note(audited(INTAKE_KEY), 93_039_215)],
        },
    ]
}

async fn register(db: &PgPool, account: &str, key: &str, leaf: u8) -> TestResult {
    users::insert(
        db,
        &NewUser {
            stellar_account: AccountId::parse(account)?,
            note_public_key: field(key),
            encryption_public_key: field(&format!("0x{}", "01".repeat(32))),
            asp_leaf: field(&format!("0x{leaf:02x}{}", "00".repeat(31))),
            kyc_scenario: KycScenario::Approved,
            kyc_status: KycStatus::Approved,
            asp_status: AspStatus::Allowed,
        },
    )
    .await?;
    Ok(())
}

fn hash(byte: u8) -> TxHash {
    TxHash::from_bytes([byte; 32])
}

fn audit_request(query: &str, token: Option<&str>) -> Result<Request<Body>, axum::http::Error> {
    let mut request = Request::get(format!("/v1/audit/transactions{query}"));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    request.body(Body::empty())
}

#[sqlx::test]
async fn decrypts_and_attributes_every_note_to_its_user(db: PgPool) -> TestResult {
    register(&db, BOB, BOB_KEY, 1).await?;
    register(&db, INTAKE, INTAKE_KEY, 2).await?;
    let explorer = Arc::new(FakeExplorer::default());
    explorer.set_ledger(10, vec![hash(0xd4)]);
    explorer.set_ledger(20, vec![hash(0x5d)]);
    explorer.set_ledger(30, vec![hash(1), hash(2)]); // ambiguous
    let app = audit_app(db, Some(Arc::new(FakeAuditor::with(history()))), explorer)?;

    let (status, _, body) = send(app, audit_request("", Some(AUDITOR_TOKEN))?).await?;
    assert_eq!(status, StatusCode::OK);
    let txs = body["transactions"].as_array().ok_or("transactions")?;
    assert_eq!(txs.len(), 3);

    assert_eq!(txs[2]["kind"], "deposit");
    assert_eq!(txs[2]["ext_amount"], "9.3039215");
    assert_eq!(txs[2]["tx_hash"], hash(0xd4).as_str());
    assert_eq!(txs[2]["outputs"][0]["owner_stellar_account"], INTAKE);

    assert_eq!(txs[1]["kind"], "transfer");
    assert_eq!(txs[1]["ext_amount"], "0");
    assert_eq!(
        txs[1]["outputs"][0],
        json!({"owner_note_public_key": BOB_KEY, "owner_stellar_account": BOB, "amount": "9.3039215"})
    );

    assert_eq!(txs[0]["kind"], "withdraw");
    assert_eq!(txs[0]["ext_amount"], "-2");
    assert_eq!(
        txs[0]["tx_hash"],
        json!(null),
        "two pool transactions in the ledger"
    );
    assert_eq!(
        txs[0]["inputs"][0]["owner_stellar_account"],
        json!(null),
        "not a Shield user"
    );
    Ok(())
}

#[sqlx::test]
async fn requires_the_auditor_token(db: PgPool) -> TestResult {
    let auditor = Arc::new(FakeAuditor::with(history()));
    let app = audit_app(db, Some(auditor.clone()), Arc::new(FakeExplorer::default()))?;
    for token in [
        None,
        Some("wrong"),
        Some(common::ADMIN_TOKEN),
        Some(&AUDITOR_TOKEN[1..]),
    ] {
        let (status, _, err) = send(app.clone(), audit_request("", token)?).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
        assert_eq!(err["error"], "unauthorized");
    }
    assert!(
        auditor.limits().is_empty(),
        "the view key is never used without the token"
    );
    Ok(())
}

#[sqlx::test]
async fn validates_the_limit(db: PgPool) -> TestResult {
    let auditor = Arc::new(FakeAuditor::with(history()));
    let app = audit_app(db, Some(auditor.clone()), Arc::new(FakeExplorer::default()))?;
    for query in ["?limit=0", "?limit=101", "?limit=abc", "?limit=-1"] {
        let (status, _, err) =
            send(app.clone(), audit_request(query, Some(AUDITOR_TOKEN))?).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
        assert_eq!(err["error"], "invalid_limit");
    }
    send(app.clone(), audit_request("", Some(AUDITOR_TOKEN))?).await?;
    let (_, _, body) = send(app, audit_request("?limit=1", Some(AUDITOR_TOKEN))?).await?;
    assert_eq!(auditor.limits(), vec![20, 1]);
    assert_eq!(body["transactions"].as_array().map(Vec::len), Some(1));
    Ok(())
}

#[sqlx::test]
async fn is_unavailable_without_an_auditor_or_when_it_fails(db: PgPool) -> TestResult {
    let app = audit_app(db.clone(), None, Arc::new(FakeExplorer::default()))?;
    let (status, _, err) = send(app, audit_request("", Some(AUDITOR_TOKEN))?).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(err["error"], "service_unavailable");

    let auditor = Arc::new(FakeAuditor::with(history()));
    auditor.set_fails(true);
    let app = audit_app(db, Some(auditor), Arc::new(FakeExplorer::default()))?;
    let (status, _, _) = send(app, audit_request("", Some(AUDITOR_TOKEN))?).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    Ok(())
}

#[sqlx::test]
async fn a_horizon_outage_only_leaves_hashes_out_and_is_retried(db: PgPool) -> TestResult {
    let explorer = Arc::new(FakeExplorer::default());
    explorer.set_ledger(10, vec![hash(0xd4)]);
    explorer.set_fails(true);
    let app = audit_app(
        db,
        Some(Arc::new(FakeAuditor::with(history()))),
        explorer.clone(),
    )?;

    let (status, _, body) = send(app.clone(), audit_request("", Some(AUDITOR_TOKEN))?).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["transactions"][2]["tx_hash"], json!(null));

    explorer.set_fails(false);
    let (_, _, body) = send(app.clone(), audit_request("", Some(AUDITOR_TOKEN))?).await?;
    assert_eq!(body["transactions"][2]["tx_hash"], hash(0xd4).as_str());
    let queries = explorer.ledger_queries().len();
    send(app, audit_request("", Some(AUDITOR_TOKEN))?).await?;
    // Resolved ledgers are cached; only the ledger without a pool tx is asked again.
    assert_eq!(explorer.ledger_queries().len(), queries + 2);
    Ok(())
}

#[sqlx::test]
async fn shows_the_public_view_without_a_token(db: PgPool) -> TestResult {
    let explorer = Arc::new(FakeExplorer::default());
    explorer.add_tx(PublicTx {
        hash: hash(0x5d),
        ledger: 5_391_876,
        created_at: Utc
            .with_ymd_and_hms(2026, 10, 26, 2, 52, 9)
            .single()
            .ok_or("time")?,
        source_account: AccountId::parse(INTAKE)?,
        is_pool_transaction: true,
        pool_ext_stroops: Some(0),
        visible_accounts: vec![AccountId::parse(INTAKE)?],
    });
    let app = audit_app(db, None, explorer.clone())?;

    let (status, _, body) = send(
        app.clone(),
        get(&format!("/v1/audit/public/{}", hash(0x5d)))?,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "tx_hash": hash(0x5d).as_str(),
            "ledger": 5_391_876,
            "created_at": "2026-10-26T02:52:09Z",
            "source_account": INTAKE,
            "is_pool_transaction": true,
            "ext_amount": "0",
            "visible_accounts": [INTAKE],
            "explorer_url": format!("https://stellar.expert/explorer/testnet/tx/{}", hash(0x5d)),
        })
    );

    let (status, _, _) = send(app.clone(), get(&format!("/v1/audit/public/{}", hash(9)))?).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, err) = send(app.clone(), get("/v1/audit/public/not-a-hash")?).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"], "invalid_tx_hash");

    explorer.set_fails(true);
    let (status, _, _) = send(app, get(&format!("/v1/audit/public/{}", hash(0x5d)))?).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    Ok(())
}
