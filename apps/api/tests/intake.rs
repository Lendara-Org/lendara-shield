//! The intake's initialisation: `CliPoolClient::check_intake` against a fake
//! CLI and a mocked RPC (a `LeafAdded` event recorded from testnet), and
//! `init_intake` with the fake ASP contract.

use std::{path::PathBuf, time::Duration};

use serde_json::json;
use shield_api::{
    adapters::{pool_cli::CliPoolClient, soroban::rpc::SorobanRpc},
    asp::AspAction,
    domain::{ContractId, PoolField},
    intake::{Confirmation, IntakeInit, IntakeInitError, init_intake},
    testing::{ChainBehaviour, FakeAspContract},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MEMBERSHIP: &str = "CCETAQARCUMNMCLJKQM6UNY2QJDOJZYGI54MXA3FMPJY4VJDMN2L26GB";
const DEPLOYMENT_LEDGER: u32 = 5_395_540;
/// The leaf of the recorded event; the fake CLI answers it in mode `ok`.
const LISTED_LEAF: &str = "0x2e20c54a6631dec2c7a35dfcca7f15907a81bac53c6b7a959bb21521a10b20b7";
const LEAF_ADDED_TOPIC: &str = "AAAADwAAAAlMZWFmQWRkZWQAAAA=";
const LEAF_ADDED_VALUE: &str = "AAAAEQAAAAEAAAADAAAADwAAAAVpbmRleAAAAAAAAAUAAAAAAAAAAAAAAA8AAAAEbGVhZgAAAAsuIMVKZjHewsejXfzKfxWQeoG6xTxrepWbshUhoQsgtwAAAA8AAAAEcm9vdAAAAAsbYX+hXJm/P5e0v1/QF55LbRz1rJd6NXxOiHJ0U18BUQ==";

fn page(events: &[serde_json::Value], cursor: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "jsonrpc": "2.0", "id": 1,
        "result": { "events": events, "cursor": cursor, "latestLedger": 5_400_000 },
    }))
}

/// An RPC whose first page (from the deployment ledger) holds the recorded
/// event and whose next page is empty.
async fn rpc() -> MockServer {
    let server = MockServer::start().await;
    let event =
        json!({ "ledger": 5_395_550, "topic": [LEAF_ADDED_TOPIC], "value": LEAF_ADDED_VALUE });
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method": "getEvents",
            "params": { "startLedger": DEPLOYMENT_LEDGER, "filters": [{ "contractIds": [MEMBERSHIP] }] },
        })))
        .respond_with(page(&[event], "c1"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method": "getEvents",
            "params": { "pagination": { "cursor": "c1" } },
        })))
        .respond_with(page(&[], "c2"))
        .mount(&server)
        .await;
    server
}

/// `mode` is the fake CLI's: `ok` (listed leaf), `unlisted`, `error`.
fn client(mode: &str, rpc_url: &str) -> Result<CliPoolClient, Box<dyn std::error::Error>> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-pool-cli.sh");
    Ok(CliPoolClient::new(
        script,
        vec![
            ("SHIELD_CIRCUITS_DIR".into(), mode.into()),
            ("PATH".into(), std::env::var("PATH")?),
        ],
        Duration::from_secs(10),
        SorobanRpc::new(rpc_url)?,
        [0; 32],
    ))
}

fn membership() -> Result<ContractId, Box<dyn std::error::Error>> {
    Ok(ContractId::parse(MEMBERSHIP)?)
}

const FAST: Confirmation = Confirmation {
    poll: Duration::ZERO,
    attempts: 3,
};

#[tokio::test]
async fn check_finds_the_intake_leaf_in_the_allow_list_events() -> TestResult {
    let server = rpc().await;
    let check = client("ok", &server.uri())?
        .check_intake(&membership()?, DEPLOYMENT_LEDGER)
        .await?;
    assert!(check.allow_listed);
    assert_eq!(check.asp_leaf.as_str(), LISTED_LEAF);
    Ok(())
}

#[tokio::test]
async fn check_reads_every_page_before_reporting_a_missing_leaf() -> TestResult {
    let server = rpc().await;
    let check = client("unlisted", &server.uri())?
        .check_intake(&membership()?, DEPLOYMENT_LEDGER)
        .await?;
    assert!(!check.allow_listed);
    // An unmatched request would have been a 404 and an error.
    assert_eq!(server.received_requests().await.map_or(0, |r| r.len()), 2);
    Ok(())
}

#[tokio::test]
async fn check_reports_an_rpc_error_as_a_retryable_chain_failure() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 1,
            "error": { "code": -32600, "message": "startLedger must be within the ledger range" },
        })))
        .mount(&server)
        .await;
    let err = client("ok", &server.uri())?
        .check_intake(&membership()?, DEPLOYMENT_LEDGER)
        .await
        .err()
        .ok_or("expected an error")?;
    assert_eq!(err.code, "chain");
    assert!(err.retryable);
    assert!(err.message.contains("ledger range"), "{}", err.message);
    Ok(())
}

#[tokio::test]
async fn init_does_nothing_when_already_allow_listed() -> TestResult {
    let server = rpc().await;
    let asp = FakeAspContract::default();
    let outcome = init_intake(
        &client("ok", &server.uri())?,
        &asp,
        &membership()?,
        DEPLOYMENT_LEDGER,
        FAST,
    )
    .await?;
    assert_eq!(
        outcome,
        IntakeInit::AlreadyAllowListed {
            asp_leaf: PoolField::parse(LISTED_LEAF)?
        }
    );
    assert_eq!(asp.submits(), 0, "no transaction when nothing is missing");
    Ok(())
}

#[tokio::test]
async fn init_inserts_a_missing_leaf_once_and_waits_for_it() -> TestResult {
    let server = rpc().await;
    let asp = FakeAspContract::default();
    asp.set_behaviour(ChainBehaviour::ConfirmsAfter(2));
    let outcome = init_intake(
        &client("unlisted", &server.uri())?,
        &asp,
        &membership()?,
        DEPLOYMENT_LEDGER,
        FAST,
    )
    .await?;
    let leaf = PoolField::parse(&format!("0x{}", "11".repeat(32)))?;
    assert!(matches!(outcome, IntakeInit::Inserted { ref asp_leaf, .. } if *asp_leaf == leaf));
    let applied = asp.applied();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].action, AspAction::Allow);
    assert_eq!(applied[0].key, leaf, "the allow-list gets the ASP leaf");
    assert_eq!(asp.submits(), 1);
    Ok(())
}

#[tokio::test]
async fn init_waits_for_a_submit_that_timed_out_but_landed() -> TestResult {
    let server = rpc().await;
    let asp = FakeAspContract::default();
    asp.set_behaviour(ChainBehaviour::SubmitTimesOutButLands);
    let outcome = init_intake(
        &client("unlisted", &server.uri())?,
        &asp,
        &membership()?,
        DEPLOYMENT_LEDGER,
        FAST,
    )
    .await?;
    assert!(matches!(outcome, IntakeInit::Inserted { .. }));
    assert_eq!(asp.submits(), 1, "no resend");
    Ok(())
}

#[tokio::test]
async fn init_reports_a_rejected_or_unconfirmed_insert() -> TestResult {
    let server = rpc().await;
    let asp = FakeAspContract::default();
    asp.set_behaviour(ChainBehaviour::Rejects);
    let err = init_intake(
        &client("unlisted", &server.uri())?,
        &asp,
        &membership()?,
        DEPLOYMENT_LEDGER,
        FAST,
    )
    .await
    .err()
    .ok_or("expected an error")?;
    assert!(matches!(err, IntakeInitError::Chain(_)), "{err}");

    let asp = FakeAspContract::default();
    asp.set_behaviour(ChainBehaviour::NeverConfirms);
    let err = init_intake(
        &client("unlisted", &server.uri())?,
        &asp,
        &membership()?,
        DEPLOYMENT_LEDGER,
        FAST,
    )
    .await
    .err()
    .ok_or("expected an error")?;
    assert!(matches!(err, IntakeInitError::Unconfirmed(_)), "{err}");
    Ok(())
}

#[tokio::test]
async fn init_forwards_a_pool_cli_failure_and_writes_nothing() -> TestResult {
    let asp = FakeAspContract::default();
    let err = init_intake(
        &client("error", "http://127.0.0.1:1")?,
        &asp,
        &membership()?,
        DEPLOYMENT_LEDGER,
        FAST,
    )
    .await
    .err()
    .ok_or("expected an error")?;
    assert!(
        matches!(&err, IntakeInitError::Pool(e) if e.code == "not_allowed"),
        "{err}"
    );
    assert!(
        err.to_string().contains("ASP membership sync required"),
        "the CLI's message is shown"
    );
    assert_eq!(asp.submits(), 0);
    Ok(())
}
