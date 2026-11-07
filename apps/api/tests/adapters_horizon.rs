//! `HorizonLedger` against a mocked Horizon, with records shaped like the
//! intake's real ones on testnet.

use serde_json::{Value, json};
use shield_api::{adapters::horizon::HorizonLedger, domain::AccountId, ports::IntakeLedger};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const INTAKE: &str = "GCRN35RTRRZAIFFEMOTZFV7PNPMFOTDNXQKLMOQFSMOBU3LAON3A6NJC";
const ANCHOR: &str = "GAO7YFNVZAHK6RSKV2LNPIS3WDGCHLBRLIFQETTPE36TLFFAGENNMNT5";
const ISSUER: &str = "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5";
const PAYMENT_HASH: &str = "77f556e4fa1fb4299e52347a26f5cdc64ff1f3daa54e7fccb71b795fe4bb0712";

fn record(id: &str, kind: &str, to: Option<&str>, memo: Option<(&str, &str)>) -> Value {
    json!({
        "id": id,
        "paging_token": id,
        "type": kind,
        "transaction_hash": PAYMENT_HASH,
        "transaction_successful": true,
        "from": if kind == "payment" { json!(ANCHOR) } else { Value::Null },
        "to": to,
        "asset_type": "credit_alphanum4",
        "asset_code": "USDC",
        "asset_issuer": ISSUER,
        "amount": "9.3039215",
        "transaction": memo.map_or(json!({"memo_type": "none"}), |(t, m)| json!({"memo_type": t, "memo": m})),
    })
}

fn page(records: &[Value]) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"_embedded": {"records": records}}))
}

async fn horizon(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/accounts/{INTAKE}/payments")))
        .and(query_param("order", "asc"))
        .and(query_param("join", "transactions"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn ledger(server: &MockServer) -> Result<HorizonLedger, Box<dyn std::error::Error>> {
    Ok(HorizonLedger::new(
        &format!("{}/", server.uri()),
        AccountId::parse(INTAKE)?,
    )?)
}

#[tokio::test]
async fn keeps_only_payments_to_the_intake() -> TestResult {
    let server = horizon(page(&[
        record("1", "create_account", None, None),
        record("2", "payment", Some(INTAKE), Some(("id", "2"))),
        record("3", "invoke_host_function", None, None),
        record("4", "payment", Some(ANCHOR), Some(("id", "5"))), // outgoing
    ]))
    .await;

    let page = ledger(&server)?.payments_after(None, 100).await?;

    assert_eq!(page.payments.len(), 1);
    let payment = &page.payments[0];
    assert_eq!(payment.operation_id, "2");
    assert_eq!(payment.tx_hash.as_str(), PAYMENT_HASH);
    assert_eq!(payment.from.as_str(), ANCHOR);
    assert_eq!(
        (payment.asset_code.as_str(), payment.asset_issuer.as_str()),
        ("USDC", ISSUER)
    );
    assert_eq!(payment.amount.as_str(), "9.3039215");
    assert_eq!(payment.memo, Some(2));
    assert_eq!(
        page.next_cursor.as_deref(),
        Some("4"),
        "the cursor passes skipped records too"
    );
    Ok(())
}

#[tokio::test]
async fn reads_only_id_memos() -> TestResult {
    let server = horizon(page(&[
        record("1", "payment", Some(INTAKE), Some(("text", "2"))),
        record("2", "payment", Some(INTAKE), None),
    ]))
    .await;
    let page = ledger(&server)?.payments_after(None, 100).await?;
    assert_eq!(
        page.payments.iter().map(|p| p.memo).collect::<Vec<_>>(),
        vec![None, None]
    );
    Ok(())
}

#[tokio::test]
async fn sends_the_cursor_and_limit() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(query_param("cursor", "23154989031505921"))
        .and(query_param("limit", "50"))
        .respond_with(page(&[]))
        .expect(1)
        .mount(&server)
        .await;

    let page = ledger(&server)?
        .payments_after(Some("23154989031505921"), 50)
        .await?;
    assert!(page.payments.is_empty());
    assert_eq!(page.next_cursor, None, "an empty page ends the poll");
    Ok(())
}

#[tokio::test]
async fn skips_failed_and_malformed_records() -> TestResult {
    let mut failed = record("1", "payment", Some(INTAKE), Some(("id", "2")));
    failed["transaction_successful"] = json!(false);
    let mut bad_amount = record("2", "payment", Some(INTAKE), Some(("id", "2")));
    bad_amount["amount"] = json!("-1");
    let server = horizon(page(&[failed, bad_amount])).await;

    let page = ledger(&server)?.payments_after(None, 100).await?;
    assert!(page.payments.is_empty());
    assert_eq!(page.next_cursor.as_deref(), Some("2"));
    Ok(())
}

#[tokio::test]
async fn reports_outages() -> TestResult {
    let server = horizon(ResponseTemplate::new(503)).await;
    assert!(ledger(&server)?.payments_after(None, 100).await.is_err());

    let server = horizon(ResponseTemplate::new(200).set_body_string("not json")).await;
    assert!(ledger(&server)?.payments_after(None, 100).await.is_err());
    Ok(())
}
