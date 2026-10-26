//! `Sep24AnchorClient` against a mocked SEP-24 anchor (wiremock): the full
//! SEP-1 → SEP-10 → SEP-24 flow, the request shapes, and every failure.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use ed25519_dalek::SigningKey;
use secrecy::SecretString;
use serde_json::{Value, json};
use shield_api::{
    adapters::anchor::{Sep24AnchorClient, Sep24AnchorConfig, sep10},
    domain::AccountId,
    ports::{AnchorClient, AnchorError, StartDeposit},
    testing::FakeClock,
};
use stellar_xdr::{
    DataValue, Limits, ManageDataOp, Memo, MuxedAccount, Operation, OperationBody, Preconditions,
    ReadXdr, SequenceNumber, String64, TimeBounds, TimePoint, Transaction, TransactionEnvelope,
    TransactionExt, TransactionV1Envelope, Uint256, VecM, WriteXdr,
};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{header, method, path},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PASSPHRASE: &str = "Test SDF Network ; September 2015";
const ISSUER: &str = "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5";
const NOW: i64 = 1_800_000_000;
const TOKEN: &str = "test-jwt";

fn server_key() -> SigningKey {
    SigningKey::from_bytes(&[1; 32])
}

fn intake_key() -> SigningKey {
    SigningKey::from_bytes(&[2; 32])
}

fn strkey(key: &SigningKey) -> String {
    stellar_strkey::ed25519::PublicKey(key.verifying_key().to_bytes())
        .to_string()
        .to_string()
}

fn home(server: &MockServer) -> String {
    server.uri().trim_start_matches("http://").to_owned()
}

fn data_op(
    source: &SigningKey,
    name: &str,
    value: &[u8],
) -> Result<Operation, Box<dyn std::error::Error>> {
    Ok(Operation {
        source_account: Some(MuxedAccount::Ed25519(Uint256(
            source.verifying_key().to_bytes(),
        ))),
        body: OperationBody::ManageData(ManageDataOp {
            data_name: String64::try_from(name.as_bytes().to_vec())?,
            data_value: Some(DataValue(value.to_vec().try_into()?)),
        }),
    })
}

/// A spec-compliant challenge for the intake, as the sandbox issues it.
fn challenge(home: &str, memo: u64) -> Result<String, Box<dyn std::error::Error>> {
    let now = u64::try_from(NOW)?;
    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(server_key().verifying_key().to_bytes())),
        fee: 200,
        seq_num: SequenceNumber(0),
        cond: Preconditions::Time(TimeBounds {
            min_time: TimePoint(now),
            max_time: TimePoint(now + 900),
        }),
        memo: Memo::Id(memo),
        operations: vec![
            data_op(&intake_key(), &format!("{home} auth"), &[b'n'; 64])?,
            data_op(&server_key(), "web_auth_domain", home.as_bytes())?,
        ]
        .try_into()?,
        ext: TransactionExt::V0,
    };
    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: VecM::default(),
    });
    Ok(sep10::sign(envelope, &server_key(), PASSPHRASE)?.to_xdr_base64(Limits::none())?)
}

fn toml(server: &MockServer, passphrase: &str) -> String {
    format!(
        "NETWORK_PASSPHRASE = \"{passphrase}\"\nSIGNING_KEY = \"{}\"\nWEB_AUTH_ENDPOINT = \"{uri}/auth\"\n\
         TRANSFER_SERVER_SEP0024 = \"{uri}/sep24\"\n",
        strkey(&server_key()),
        uri = server.uri()
    )
}

/// A healthy anchor. `customer_memo` is the memo the challenge carries.
async fn anchor(customer_memo: u64) -> Result<MockServer, Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/stellar.toml"))
        .respond_with(ResponseTemplate::new(200).set_body_string(toml(&server, PASSPHRASE)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/auth"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "transaction": challenge(&home(&server), customer_memo)?,
            "network_passphrase": PASSPHRASE,
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": TOKEN})))
        .mount(&server)
        .await;
    Ok(server)
}

async fn mount_deposit(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/sep24/transactions/deposit/interactive"))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .respond_with(response)
        .mount(server)
        .await;
}

fn client(server: &MockServer) -> Result<Sep24AnchorClient, Box<dyn std::error::Error>> {
    let secret = stellar_strkey::ed25519::PrivateKey(intake_key().to_bytes())
        .as_unredacted()
        .to_string()
        .to_string();
    let clock = FakeClock::at(Utc.timestamp_opt(NOW + 5, 0).single().ok_or("time")?);
    Ok(Sep24AnchorClient::new(
        Sep24AnchorConfig {
            home_domain: home(server),
            intake_secret: SecretString::from(secret),
            asset_issuer: AccountId::parse(ISSUER)?,
            network_passphrase: PASSPHRASE.into(),
        },
        Arc::new(clock),
    )?)
}

fn deposit_request() -> StartDeposit {
    StartDeposit {
        asset_code: "USDC".into(),
        source_asset: "iso4217:CRC".into(),
        customer_memo: 7,
        deposit_memo: 1001,
    }
}

fn interactive() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "type": "interactive_customer_info_needed",
        "url": "https://anchor.example/widget?token=abc",
        "id": "anchor-tx-1"
    }))
}

async fn requests_to(server: &MockServer, http_method: &str, route: &str) -> Vec<Request> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.method.as_str() == http_method && r.url.path() == route)
        .collect()
}

#[tokio::test]
async fn starts_an_interactive_deposit_to_the_intake() -> TestResult {
    let server = anchor(7).await?;
    mount_deposit(&server, interactive()).await;

    let deposit = client(&server)?.start_deposit(&deposit_request()).await?;
    assert_eq!(deposit.id, "anchor-tx-1");
    assert_eq!(
        deposit.interactive_url,
        "https://anchor.example/widget?token=abc"
    );

    // SEP-10: challenge requested for the intake with the customer memo...
    let [challenge_request] = requests_to(&server, "GET", "/auth")
        .await
        .try_into()
        .map_err(|_| "one challenge")?;
    let query: Vec<(String, String)> = challenge_request.url.query_pairs().into_owned().collect();
    assert!(query.contains(&("account".into(), strkey(&intake_key()))));
    assert!(query.contains(&("memo".into(), "7".into())));
    assert!(query.contains(&("home_domain".into(), home(&server))));

    // ...and returned signed by both the anchor and the intake.
    let [token_request] = requests_to(&server, "POST", "/auth")
        .await
        .try_into()
        .map_err(|_| "one token request")?;
    let body: Value = serde_json::from_slice(&token_request.body)?;
    let signed = TransactionEnvelope::from_xdr_base64(
        body["transaction"].as_str().ok_or("tx")?,
        Limits::none(),
    )?;
    let TransactionEnvelope::Tx(v1) = signed else {
        return Err("v1".into());
    };
    assert_eq!(v1.signatures.len(), 2);

    // SEP-24: deposit to the intake with the intent memo as an `id` memo.
    let [deposit_request] = requests_to(&server, "POST", "/sep24/transactions/deposit/interactive")
        .await
        .try_into()
        .map_err(|_| "one deposit request")?;
    let body: Value = serde_json::from_slice(&deposit_request.body)?;
    assert_eq!(
        body,
        json!({
            "asset_code": "USDC",
            "asset_issuer": ISSUER,
            "account": strkey(&intake_key()),
            "memo": "1001",
            "memo_type": "id",
            "source_asset": "iso4217:CRC",
        })
    );
    Ok(())
}

#[tokio::test]
async fn reads_the_deposit_status_as_the_customer() -> TestResult {
    let server = anchor(7).await?;
    Mock::given(method("GET"))
        .and(path("/sep24/transaction"))
        .and(wiremock::matchers::query_param("id", "anchor-tx-1"))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "transaction": {"id": "anchor-tx-1", "status": "error", "message": "KYC rejected (sandbox scenario)."}
        })))
        .mount(&server)
        .await;

    let status = client(&server)?.deposit_status(7, "anchor-tx-1").await?;
    assert_eq!(
        status,
        shield_api::ports::AnchorStatus::Failed("KYC rejected (sandbox scenario).".into())
    );
    let [challenge] = requests_to(&server, "GET", "/auth")
        .await
        .try_into()
        .map_err(|_| "one login")?;
    assert!(
        challenge
            .url
            .query_pairs()
            .any(|(k, v)| k == "memo" && v == "7")
    );
    Ok(())
}

#[tokio::test]
async fn discovers_the_anchor_once() -> TestResult {
    let server = anchor(7).await?;
    mount_deposit(&server, interactive()).await;
    let client = client(&server)?;
    client.start_deposit(&deposit_request()).await?;
    client.start_deposit(&deposit_request()).await?;

    assert_eq!(
        requests_to(&server, "GET", "/.well-known/stellar.toml")
            .await
            .len(),
        1
    );
    assert_eq!(
        requests_to(&server, "GET", "/auth").await.len(),
        2,
        "one SEP-10 login per deposit"
    );
    Ok(())
}

#[tokio::test]
async fn never_signs_a_challenge_that_breaks_the_spec() -> TestResult {
    // The anchor answers with a challenge for another customer memo.
    let server = anchor(999).await?;
    mount_deposit(&server, interactive()).await;

    let outcome = client(&server)?.start_deposit(&deposit_request()).await;
    assert!(matches!(outcome, Err(AnchorError::Refused(reason)) if reason.contains("SEP-10")));
    assert!(
        requests_to(&server, "POST", "/auth").await.is_empty(),
        "nothing was signed or sent"
    );
    Ok(())
}

#[tokio::test]
async fn refuses_an_anchor_on_another_network() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/stellar.toml"))
        .respond_with(ResponseTemplate::new(200).set_body_string(toml(
            &server,
            "Public Global Stellar Network ; September 2015",
        )))
        .mount(&server)
        .await;

    let outcome = client(&server)?.start_deposit(&deposit_request()).await;
    assert!(matches!(outcome, Err(AnchorError::Refused(reason)) if reason.contains("network")));
    Ok(())
}

#[tokio::test]
async fn classifies_deposit_failures() -> TestResult {
    let cases = [
        (ResponseTemplate::new(400).set_body_json(json!({"error": "asset not supported"})), "refused"),
        (ResponseTemplate::new(503), "unavailable"),
        (ResponseTemplate::new(200).set_body_json(json!({"unexpected": true})), "unavailable"),
        (
            ResponseTemplate::new(200).set_body_json(json!({"type": "completed", "url": "https://a.example", "id": "x"})),
            "refused",
        ),
        (
            ResponseTemplate::new(200).set_body_json(json!({"type": "interactive_customer_info_needed", "url": "http://evil.example", "id": "x"})),
            "refused",
        ),
    ];
    for (response, expected) in cases {
        let server = anchor(7).await?;
        mount_deposit(&server, response).await;
        let outcome = client(&server)?.start_deposit(&deposit_request()).await;
        match expected {
            "refused" => assert!(
                matches!(outcome, Err(AnchorError::Refused(_))),
                "{outcome:?}"
            ),
            _ => assert!(
                matches!(outcome, Err(AnchorError::Unavailable(_))),
                "{outcome:?}"
            ),
        }
    }
    Ok(())
}

#[tokio::test]
async fn an_unreachable_anchor_is_temporary() -> TestResult {
    // Nothing listens on port 1: connection refused.
    let client = Sep24AnchorClient::new(
        Sep24AnchorConfig {
            home_domain: "127.0.0.1:1".into(),
            intake_secret: SecretString::from(
                stellar_strkey::ed25519::PrivateKey(intake_key().to_bytes())
                    .as_unredacted()
                    .to_string()
                    .to_string(),
            ),
            asset_issuer: AccountId::parse(ISSUER)?,
            network_passphrase: PASSPHRASE.into(),
        },
        Arc::new(FakeClock::at(Utc::now())),
    )?;

    assert!(matches!(
        client.start_deposit(&deposit_request()).await,
        Err(AnchorError::Unavailable(_))
    ));
    Ok(())
}

#[tokio::test]
async fn never_prints_the_intake_secret() -> TestResult {
    let server = MockServer::start().await;
    let secret = stellar_strkey::ed25519::PrivateKey(intake_key().to_bytes())
        .as_unredacted()
        .to_string()
        .to_string();
    let client = client(&server)?;
    assert!(!format!("{client:?}").contains(secret.as_str()));
    assert!(
        Sep24AnchorClient::new(
            Sep24AnchorConfig {
                home_domain: home(&server),
                intake_secret: SecretString::from("SNOPE"),
                asset_issuer: AccountId::parse(ISSUER)?,
                network_passphrase: PASSPHRASE.into(),
            },
            Arc::new(FakeClock::at(Utc::now())),
        )
        .is_err()
    );
    Ok(())
}
