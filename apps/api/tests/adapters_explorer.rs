//! `HorizonExplorer` against a mocked Horizon serving real testnet records
//! (public data): carol's direct deposit and the private transfer of her
//! ramp deposit, and a busy ledger with other contracts' calls.

use serde_json::Value;
use shield_api::{
    adapters::explorer::HorizonExplorer,
    domain::{AccountId, ContractId, TxHash},
    ports::Explorer,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const POOL: &str = "CCUJRW6VOQBFT5EWSKTYF24WHLFQFTO6SH33VFWAGOJFFSYGXPWXK4HF";
const CAROL: &str = "GCWQUSCIB2S4LNW6FIOKL2N6BLFKAQ365A43IZVWJBR2ANAEDSIG3GQK";
const INTAKE: &str = "GCRN35RTRRZAIFFEMOTZFV7PNPMFOTDNXQKLMOQFSMOBU3LAON3A6NJC";
const DEPOSIT: &str = "a907e81ed27284dc8621c8eb21c9b5c3ffc48d1d3f24556e877b979b2f646d46";
const TRANSFER: &str = "2fe1ff4ff0891a586cac16d2632aeb78a43f90710bea7627a9230d8113ecf6e1";

fn fixture(name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let file = format!(
        "{}/tests/fixtures/horizon/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    Ok(serde_json::from_str(&std::fs::read_to_string(file)?)?)
}

async fn serve(server: &MockServer, url: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(url))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn horizon_with(hash: &str, name: &str) -> Result<MockServer, Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    serve(
        &server,
        &format!("/transactions/{hash}"),
        fixture(&format!("{name}.tx"))?,
    )
    .await;
    serve(
        &server,
        &format!("/transactions/{hash}/operations"),
        fixture(&format!("{name}.operations"))?,
    )
    .await;
    serve(
        &server,
        &format!("/transactions/{hash}/effects"),
        fixture(&format!("{name}.effects"))?,
    )
    .await;
    Ok(server)
}

fn explorer(server: &MockServer) -> Result<HorizonExplorer, Box<dyn std::error::Error>> {
    Ok(HorizonExplorer::new(
        &format!("{}/", server.uri()),
        ContractId::parse(POOL)?,
    )?)
}

fn accounts(tx: &shield_api::ports::PublicTx) -> Vec<&str> {
    tx.visible_accounts.iter().map(AccountId::as_str).collect()
}

#[tokio::test]
async fn a_direct_deposit_shows_the_depositor_and_the_amount() -> TestResult {
    let server = horizon_with(DEPOSIT, "direct-deposit").await?;
    let tx = explorer(&server)?
        .transaction(&TxHash::parse(DEPOSIT)?)
        .await?
        .ok_or("known transaction")?;

    assert!(tx.is_pool_transaction);
    assert_eq!(tx.pool_ext_stroops, Some(20_000_000));
    assert_eq!(tx.source_account.as_str(), CAROL);
    assert_eq!(tx.ledger, 5_392_288);
    assert!(accounts(&tx).contains(&CAROL));
    Ok(())
}

#[tokio::test]
async fn a_ramp_transfer_hides_the_user() -> TestResult {
    let server = horizon_with(TRANSFER, "ramp-transfer").await?;
    let tx = explorer(&server)?
        .transaction(&TxHash::parse(TRANSFER)?)
        .await?
        .ok_or("known transaction")?;

    assert!(tx.is_pool_transaction);
    assert_eq!(
        tx.pool_ext_stroops,
        Some(0),
        "a private transfer moves nothing publicly"
    );
    assert_eq!(tx.source_account.as_str(), INTAKE);
    assert!(accounts(&tx).contains(&INTAKE));
    assert!(
        !accounts(&tx).contains(&CAROL),
        "the recipient is nowhere in it"
    );
    Ok(())
}

#[tokio::test]
async fn an_unknown_transaction_is_none() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    assert!(
        explorer(&server)?
            .transaction(&TxHash::parse(DEPOSIT)?)
            .await?
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn finds_only_the_pools_transaction_in_a_busy_ledger() -> TestResult {
    let server = MockServer::start().await;
    serve(
        &server,
        "/ledgers/5392310/operations",
        fixture("ledger-ops")?,
    )
    .await;
    let hashes = explorer(&server)?
        .pool_transactions_in_ledger(5_392_310)
        .await?;
    assert_eq!(hashes, vec![TxHash::parse(TRANSFER)?]);
    Ok(())
}

#[tokio::test]
async fn reports_outages() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let explorer = explorer(&server)?;
    assert!(
        explorer
            .transaction(&TxHash::parse(DEPOSIT)?)
            .await
            .is_err()
    );
    assert!(explorer.pool_transactions_in_ledger(1).await.is_err());
    Ok(())
}
