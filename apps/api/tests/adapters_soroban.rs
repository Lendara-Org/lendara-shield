//! `SorobanAspContract` against a mocked Stellar RPC (wiremock) answering
//! with real XDR: request shape, sequence handling, signing and every
//! response status.

use std::{sync::Arc, time::Duration};

use chrono::{TimeZone, Utc};
use ed25519_dalek::{Signature, SigningKey, Verifier};
use secrecy::SecretString;
use serde_json::{Value, json};
use shield_api::{
    adapters::soroban::{SorobanAspConfig, SorobanAspContract},
    asp::AspAction,
    domain::{ContractId, PoolField, TxHash},
    ports::{AspContract, AspKeys, ChainError, TxStatus},
    testing::FakeClock,
};
use stellar_xdr::{
    AccountEntry, AccountEntryExt, AccountId, HostFunction, LedgerEntryData, LedgerFootprint,
    Limits, OperationBody, PublicKey, ReadXdr, ScVal, SequenceNumber, SorobanResources,
    SorobanTransactionData, SorobanTransactionDataExt, String32, Thresholds, TransactionEnvelope,
    TransactionExt, Uint256, VecM, WriteXdr,
};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{body_partial_json, method},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PASSPHRASE: &str = "Test SDF Network ; September 2015";
const MEMBERSHIP: &str = "CDUJHXHC2DXXOXKBIK5E5QDKD6C3WUSMDIUFF6QTCECBNI4S2656VYD6";
const NON_MEMBERSHIP: &str = "CA46MJAIJTYJ5CO4OLRYRRTRJ4MVGO55LB6RQA6756FUNZPU2UTRD463";
const ON_CHAIN_SEQUENCE: i64 = 100;
const SEED: [u8; 32] = [7; 32];

fn operator() -> SigningKey {
    SigningKey::from_bytes(&SEED)
}

fn keys() -> Result<AspKeys, Box<dyn std::error::Error>> {
    Ok(AspKeys {
        asp_leaf: PoolField::parse(&format!("0x{}", "05".repeat(32)))?,
        note_public_key: PoolField::parse(&format!("0x{}{}", "11".repeat(31), "22"))?,
    })
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0)
        .single()
        .unwrap_or_default()
}

fn adapter(server: &MockServer) -> Result<SorobanAspContract, Box<dyn std::error::Error>> {
    let secret = stellar_strkey::ed25519::PrivateKey(SEED)
        .as_unredacted()
        .to_string()
        .to_string();
    Ok(SorobanAspContract::new(
        SorobanAspConfig {
            rpc_url: server.uri(),
            network_passphrase: PASSPHRASE.into(),
            operator_secret: SecretString::from(secret.as_str()),
            asp_membership: ContractId::parse(MEMBERSHIP)?,
            asp_non_membership: ContractId::parse(NON_MEMBERSHIP)?,
            validity: Duration::from_secs(60),
        },
        Arc::new(FakeClock::at(now())),
    )?)
}

fn rpc_result(result: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"jsonrpc": "2.0", "id": 1, "result": result}))
}

fn account_entry_xdr() -> Result<String, stellar_xdr::Error> {
    LedgerEntryData::Account(AccountEntry {
        account_id: AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            operator().verifying_key().to_bytes(),
        ))),
        balance: 100_000_000,
        seq_num: SequenceNumber(ON_CHAIN_SEQUENCE),
        num_sub_entries: 0,
        inflation_dest: None,
        flags: 0,
        home_domain: String32::default(),
        thresholds: Thresholds([1, 0, 0, 0]),
        signers: VecM::default(),
        ext: AccountEntryExt::V0,
    })
    .to_xdr_base64(Limits::none())
}

fn transaction_data_xdr() -> Result<String, stellar_xdr::Error> {
    SorobanTransactionData {
        ext: SorobanTransactionDataExt::V0,
        resources: SorobanResources {
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: VecM::default(),
            },
            instructions: 1_000,
            disk_read_bytes: 100,
            write_bytes: 100,
        },
        resource_fee: 50_000,
    }
    .to_xdr_base64(Limits::none())
}

/// A healthy RPC: account, simulation, send and get all succeed.
async fn healthy_rpc() -> Result<MockServer, Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    mount(
        &server,
        "getLedgerEntries",
        json!({"entries": [{"xdr": account_entry_xdr()?}], "latestLedger": 10}),
    )
    .await;
    mount(
        &server,
        "simulateTransaction",
        json!({
            "transactionData": transaction_data_xdr()?,
            "minResourceFee": "50000",
            "results": [{"auth": [], "xdr": "AAAAAQ=="}],
            "latestLedger": 10
        }),
    )
    .await;
    Ok(server)
}

async fn mount(server: &MockServer, rpc_method: &str, result: Value) {
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method": rpc_method})))
        .respond_with(rpc_result(&result))
        .mount(server)
        .await;
}

/// The envelope sent in a recorded request to `rpc_method`.
async fn sent_envelopes(
    server: &MockServer,
    rpc_method: &str,
) -> Result<Vec<TransactionEnvelope>, Box<dyn std::error::Error>> {
    let requests: Vec<Request> = server.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter_map(|r| serde_json::from_slice::<Value>(&r.body).ok())
        .filter(|body| body["method"] == rpc_method)
        .map(|body| {
            let b64 = body["params"]["transaction"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            Ok(TransactionEnvelope::from_xdr_base64(&b64, Limits::none())?)
        })
        .collect()
}

/// (contract strkey, function, args) of an envelope's single invocation.
fn invocation(
    envelope: &TransactionEnvelope,
) -> Result<(String, String, Vec<ScVal>), Box<dyn std::error::Error>> {
    let TransactionEnvelope::Tx(v1) = envelope else {
        return Err("expected a v1 envelope".into());
    };
    let [op] = v1.tx.operations.as_slice() else {
        return Err("expected one operation".into());
    };
    let OperationBody::InvokeHostFunction(invoke) = &op.body else {
        return Err("expected an invocation".into());
    };
    let HostFunction::InvokeContract(args) = &invoke.host_function else {
        return Err("expected a contract call".into());
    };
    let stellar_xdr::ScAddress::Contract(contract) = &args.contract_address else {
        return Err("expected a contract address".into());
    };
    Ok((
        stellar_strkey::Contract(contract.0.0)
            .to_string()
            .to_string(),
        args.function_name.to_utf8_string()?,
        args.args.to_vec(),
    ))
}

#[tokio::test]
async fn allow_inserts_the_leaf_into_the_membership_tree() -> TestResult {
    let server = healthy_rpc().await?;
    let prepared = adapter(&server)?
        .prepare(AspAction::Allow, &keys()?)
        .await?;

    let envelope = TransactionEnvelope::from_xdr_base64(&prepared.envelope_xdr, Limits::none())?;
    let (contract, function, args) = invocation(&envelope)?;
    assert_eq!(contract, MEMBERSHIP);
    assert_eq!(function, "insert_leaf");
    assert_eq!(
        args,
        vec![shield_api::adapters::soroban::tx::u256(
            keys()?.asp_leaf.field_be()
        )]
    );

    let TransactionEnvelope::Tx(v1) = &envelope else {
        return Err("expected a v1 envelope".into());
    };
    assert_eq!(v1.tx.seq_num, SequenceNumber(ON_CHAIN_SEQUENCE + 1));
    assert!(
        matches!(v1.tx.ext, TransactionExt::V1(_)),
        "simulation data applied"
    );
    assert_eq!(prepared.valid_until, now() + chrono::Duration::seconds(60));

    // The returned hash is the network hash of the signed envelope, and the
    // operator's signature verifies against it.
    let hash = envelope.hash(shield_api::adapters::soroban::tx::network_id(PASSPHRASE))?;
    assert_eq!(prepared.hash, TxHash::from_bytes(hash));
    let [decorated] = v1.signatures.as_slice() else {
        return Err("expected one signature".into());
    };
    operator().verifying_key().verify(
        &hash,
        &Signature::from_slice(decorated.signature.0.as_slice())?,
    )?;
    Ok(())
}

#[tokio::test]
async fn block_and_unblock_target_the_non_membership_tree() -> TestResult {
    let server = healthy_rpc().await?;
    let adapter = adapter(&server)?;
    // Block-list key: the note key's bytes reversed (little-endian field).
    let note_key = shield_api::adapters::soroban::tx::u256(keys()?.note_public_key.field_le());

    // Explicit: the note key 0x1111…1122 becomes the block-list key 0x2211…11.
    let ScVal::U256(parts) = &note_key else {
        return Err("expected a U256".into());
    };
    assert_eq!(parts.hi_hi, 0x2211_1111_1111_1111);
    assert_eq!(parts.lo_lo, 0x1111_1111_1111_1111);

    let block = adapter.prepare(AspAction::Block, &keys()?).await?;
    let unblock = adapter.prepare(AspAction::Unblock, &keys()?).await?;

    let decode = |b64: &str| TransactionEnvelope::from_xdr_base64(b64, Limits::none());
    assert_eq!(
        invocation(&decode(&block.envelope_xdr)?)?,
        (
            NON_MEMBERSHIP.to_owned(),
            "insert_leaf".to_owned(),
            vec![note_key.clone(), note_key.clone()]
        )
    );
    assert_eq!(
        invocation(&decode(&unblock.envelope_xdr)?)?,
        (
            NON_MEMBERSHIP.to_owned(),
            "delete_leaf".to_owned(),
            vec![note_key]
        )
    );
    Ok(())
}

#[tokio::test]
async fn consecutive_prepares_take_consecutive_sequence_numbers() -> TestResult {
    let server = healthy_rpc().await?;
    let adapter = adapter(&server)?;
    adapter.prepare(AspAction::Allow, &keys()?).await?;
    adapter.prepare(AspAction::Allow, &keys()?).await?;

    // The network still reports 100: the adapter must not reuse 101.
    let sequences: Vec<i64> = sent_envelopes(&server, "simulateTransaction")
        .await?
        .iter()
        .filter_map(|e| match e {
            TransactionEnvelope::Tx(v1) => Some(v1.tx.seq_num.0),
            _ => None,
        })
        .collect();
    assert_eq!(sequences, vec![101, 102]);
    Ok(())
}

#[tokio::test]
async fn submit_maps_every_send_status() -> TestResult {
    let cases = [
        ("PENDING", None),
        ("DUPLICATE", None),
        ("TRY_AGAIN_LATER", Some("unavailable")),
        ("ERROR", Some("rejected")),
    ];
    for (status, expected) in cases {
        let server = healthy_rpc().await?;
        mount(
            &server,
            "sendTransaction",
            json!({"status": status, "hash": "ab", "latestLedger": 10, "errorResultXdr": "AAAA"}),
        )
        .await;
        let adapter = adapter(&server)?;
        let prepared = adapter.prepare(AspAction::Allow, &keys()?).await?;

        let outcome = adapter.submit(&prepared).await;
        match expected {
            None => assert!(outcome.is_ok(), "{status}: {outcome:?}"),
            Some("unavailable") => assert!(
                matches!(outcome, Err(ChainError::Unavailable(_))),
                "{status}"
            ),
            Some(_) => assert!(matches!(outcome, Err(ChainError::Rejected(_))), "{status}"),
        }
    }
    Ok(())
}

#[tokio::test]
async fn a_refused_transaction_frees_its_sequence_number() -> TestResult {
    let server = healthy_rpc().await?;
    mount(
        &server,
        "sendTransaction",
        json!({"status": "ERROR", "hash": "ab", "latestLedger": 10, "errorResultXdr": "AAAA"}),
    )
    .await;
    let adapter = adapter(&server)?;

    let first = adapter.prepare(AspAction::Allow, &keys()?).await?;
    assert!(adapter.submit(&first).await.is_err());
    adapter.prepare(AspAction::Allow, &keys()?).await?;

    let sequences: Vec<i64> = sent_envelopes(&server, "simulateTransaction")
        .await?
        .iter()
        .filter_map(|e| match e {
            TransactionEnvelope::Tx(v1) => Some(v1.tx.seq_num.0),
            _ => None,
        })
        .collect();
    assert_eq!(
        sequences,
        vec![101, 101],
        "101 was never used, so it is reused"
    );
    Ok(())
}

#[tokio::test]
async fn status_maps_every_transaction_status() -> TestResult {
    let hash = TxHash::from_bytes([9; 32]);
    let cases = [
        ("SUCCESS", TxStatus::Success),
        ("FAILED", TxStatus::Failed("AAAB".into())),
        ("NOT_FOUND", TxStatus::NotFound),
    ];
    for (status, expected) in cases {
        let server = MockServer::start().await;
        mount(
            &server,
            "getTransaction",
            json!({"status": status, "resultXdr": "AAAB", "latestLedger": 10}),
        )
        .await;
        assert_eq!(adapter(&server)?.status(&hash).await?, expected, "{status}");
    }
    Ok(())
}

#[tokio::test]
async fn a_failing_simulation_is_a_definitive_rejection() -> TestResult {
    let server = MockServer::start().await;
    mount(
        &server,
        "getLedgerEntries",
        json!({"entries": [{"xdr": account_entry_xdr()?}], "latestLedger": 10}),
    )
    .await;
    mount(
        &server,
        "simulateTransaction",
        json!({"error": "HostError: Error(Contract, #3)", "latestLedger": 10}),
    )
    .await;

    let outcome = adapter(&server)?.prepare(AspAction::Block, &keys()?).await;
    assert!(matches!(outcome, Err(ChainError::Rejected(reason)) if reason.contains("Contract")));
    Ok(())
}

#[tokio::test]
async fn rpc_outages_are_temporary() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(matches!(
        adapter(&server)?.prepare(AspAction::Allow, &keys()?).await,
        Err(ChainError::Unavailable(_))
    ));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32600, "message": "bad"}}),
        ))
        .mount(&server)
        .await;
    assert!(matches!(
        adapter(&server)?.status(&TxHash::from_bytes([1; 32])).await,
        Err(ChainError::Unavailable(_))
    ));
    Ok(())
}

#[tokio::test]
async fn an_unfunded_operator_is_reported() -> TestResult {
    let server = MockServer::start().await;
    mount(
        &server,
        "getLedgerEntries",
        json!({"entries": [], "latestLedger": 10}),
    )
    .await;
    let outcome = adapter(&server)?.prepare(AspAction::Allow, &keys()?).await;
    assert!(
        matches!(outcome, Err(ChainError::Unavailable(reason)) if reason.contains("not found"))
    );
    Ok(())
}

#[tokio::test]
async fn rejects_an_invalid_secret_and_never_prints_it() -> TestResult {
    let server = MockServer::start().await;
    let config = |secret: &str| -> Result<SorobanAspConfig, Box<dyn std::error::Error>> {
        Ok(SorobanAspConfig {
            rpc_url: server.uri(),
            network_passphrase: PASSPHRASE.into(),
            operator_secret: SecretString::from(secret),
            asp_membership: ContractId::parse(MEMBERSHIP)?,
            asp_non_membership: ContractId::parse(NON_MEMBERSHIP)?,
            validity: Duration::from_secs(60),
        })
    };
    assert!(SorobanAspContract::new(config("SNOTASEED")?, Arc::new(FakeClock::at(now()))).is_err());

    let secret = stellar_strkey::ed25519::PrivateKey(SEED)
        .as_unredacted()
        .to_string()
        .to_string();
    let valid = config(secret.as_str())?;
    assert!(!format!("{valid:?}").contains(secret.as_str()));
    let adapter = SorobanAspContract::new(valid, Arc::new(FakeClock::at(now())))?;
    assert!(!format!("{adapter:?}").contains(secret.as_str()));
    assert!(adapter.operator_address().starts_with('G'));
    Ok(())
}
