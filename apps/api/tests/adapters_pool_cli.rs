//! `CliPoolClient` against a fake CLI script: the subprocess plumbing
//! (arguments, environment isolation, exit codes, protocol, timeouts).

use std::{path::PathBuf, time::Duration};

use shield_api::{
    adapters::{pool_cli::CliPoolClient, soroban::rpc::SorobanRpc},
    domain::{PoolField, TxHash},
    ports::{PoolClient, TxStatus},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn client(mode: &str, timeout: Duration) -> Result<CliPoolClient, Box<dyn std::error::Error>> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-pool-cli.sh");
    Ok(CliPoolClient::new(
        script,
        // The fake reads its failure mode from a forwarded variable.
        vec![
            ("SHIELD_CIRCUITS_DIR".into(), mode.into()),
            ("PATH".into(), std::env::var("PATH")?),
        ],
        timeout,
        SorobanRpc::new("http://127.0.0.1:1")?,
        [0; 32],
    ))
}

#[tokio::test]
async fn prepares_with_the_right_arguments_and_no_leaked_env() -> TestResult {
    // A variable set in the API process must not reach the CLI.
    let tx = client("ok", Duration::from_secs(10))?
        .prepare_deposit(93_039_215)
        .await?;
    assert_eq!(tx.hash, TxHash::from_bytes([0xab; 32]));
    assert_eq!(tx.sequence, 42);
    assert_eq!(tx.envelope_xdr, "prepare-deposit:93039215:none");

    let key = PoolField::parse(&format!("0x{}", "11".repeat(32)))?;
    let tx = client("ok", Duration::from_secs(10))?
        .prepare_transfer(5, &key, &key)
        .await?;
    assert_eq!(tx.envelope_xdr, "prepare-transfer:5:none");
    Ok(())
}

#[tokio::test]
async fn maps_submit_and_status() -> TestResult {
    let client = client("ok", Duration::from_secs(10))?;
    client.submit("AAAA").await?;
    assert_eq!(
        client.status(&TxHash::from_bytes([1; 32])).await?,
        TxStatus::Failed("AAAA".into())
    );
    Ok(())
}

#[tokio::test]
async fn forwards_cli_errors_with_their_retry_hint() -> TestResult {
    let err = client("error", Duration::from_secs(10))?
        .prepare_deposit(1)
        .await
        .err()
        .ok_or("expected an error")?;
    assert_eq!(err.code, "not_allowed");
    assert!(!err.retryable);
    Ok(())
}

#[tokio::test]
async fn protocol_violations_are_retryable_failures() -> TestResult {
    for mode in ["garbage", "crash"] {
        let err = client(mode, Duration::from_secs(10))?
            .prepare_deposit(1)
            .await
            .err()
            .ok_or("expected an error")?;
        assert_eq!(err.code, "cli_protocol", "{mode}");
        assert!(err.retryable, "{mode}");
    }
    Ok(())
}

#[tokio::test]
async fn a_hung_cli_times_out() -> TestResult {
    let err = client("slow", Duration::from_millis(300))?
        .prepare_deposit(1)
        .await
        .err()
        .ok_or("expected an error")?;
    assert_eq!(err.code, "cli_timeout");
    assert!(err.retryable);
    Ok(())
}

#[tokio::test]
async fn a_missing_binary_is_reported() -> TestResult {
    let client = CliPoolClient::new(
        PathBuf::from("/nonexistent/pool-cli"),
        Vec::new(),
        Duration::from_secs(1),
        SorobanRpc::new("http://127.0.0.1:1")?,
        [0; 32],
    );
    let err = client
        .prepare_deposit(1)
        .await
        .err()
        .ok_or("expected an error")?;
    assert_eq!(err.code, "cli_unavailable");
    assert!(client.intake_sequence().await.is_err(), "unreachable RPC");
    Ok(())
}
