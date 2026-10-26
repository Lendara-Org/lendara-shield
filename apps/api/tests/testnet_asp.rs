//! Against the real testnet deployment (`make test-testnet`; needs `.env`).
//! Read-only: it prepares (simulates and signs) an ASP write but never sends
//! it, so the real allow-list is not polluted.

use std::{path::Path, sync::Arc};

use secrecy::SecretString;
use shield_api::{
    adapters::soroban::{DEFAULT_VALIDITY, SorobanAspConfig, SorobanAspContract},
    asp::AspAction,
    deployment::PoolDeployment,
    domain::PoolField,
    ports::{AspContract, AspKeys, SystemClock, TxStatus},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is not set; run make setup-testnet"))
}

#[tokio::test]
#[ignore = "talks to Stellar testnet; run with `make test-testnet`"]
async fn testnet_prepares_an_allow_list_insert_without_sending_it() -> TestResult {
    let deployment = PoolDeployment::load(Path::new(&env("SHIELD_DEPLOYMENT_FILE")?))?;
    let adapter = SorobanAspContract::new(
        SorobanAspConfig {
            rpc_url: std::env::var("SOROBAN_RPC_URL")
                .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".into()),
            network_passphrase: "Test SDF Network ; September 2015".into(),
            operator_secret: SecretString::from(env("SHIELD_OPERATOR_SECRET")?),
            asp_membership: deployment.asp_membership,
            asp_non_membership: deployment.asp_non_membership,
            validity: DEFAULT_VALIDITY,
        },
        Arc::new(SystemClock),
    )?;
    let field = PoolField::parse(&format!("0x{}", "0d".repeat(32)))?;
    let keys = AspKeys {
        asp_leaf: field.clone(),
        note_public_key: field,
    };

    // Simulation succeeds only if the operator is the trees' admin and the
    // call is well formed.
    let prepared = adapter.prepare(AspAction::Allow, &keys).await?;
    assert_eq!(prepared.hash.as_str().len(), 64);

    // Never sent, so the network does not know it.
    assert_eq!(adapter.status(&prepared.hash).await?, TxStatus::NotFound);
    Ok(())
}

#[tokio::test]
#[ignore = "talks to Stellar testnet; run with `make test-testnet`"]
async fn testnet_the_intake_is_initialised() -> TestResult {
    use shield_api::adapters::{
        account_of,
        pool_cli::{self, CliPoolClient},
        soroban::{allow_list, rpc::SorobanRpc},
    };

    let deployment = PoolDeployment::load(Path::new(&env("SHIELD_DEPLOYMENT_FILE")?))?;
    let rpc = SorobanRpc::new(
        std::env::var("SOROBAN_RPC_URL")
            .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".into()),
    )?;
    // The real event history decodes, and a leaf nobody inserted is absent.
    let nobody = PoolField::parse(&format!("0x{}", "11".repeat(32)))?;
    assert!(
        !allow_list::contains_leaf(
            &rpc,
            &deployment.asp_membership,
            deployment.deployment_ledger,
            &nobody
        )
        .await?
    );

    // Derives the intake's keys into its local wallet if missing (no
    // transaction) and checks its leaf is allow-listed.
    let intake = account_of(&SecretString::from(env("SHIELD_INTAKE_SECRET")?))
        .ok_or("SHIELD_INTAKE_SECRET is not a valid secret seed")?;
    let client = CliPoolClient::new(
        env("POOL_CLI_PATH")?.into(),
        CliPoolClient::forwarded_env(),
        pool_cli::DEFAULT_TIMEOUT,
        rpc,
        stellar_strkey::ed25519::PublicKey::from_string(intake.as_str())?.0,
    );
    let check = client
        .check_intake(&deployment.asp_membership, deployment.deployment_ledger)
        .await
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    assert!(
        check.allow_listed,
        "intake leaf {} is not allow-listed: run scripts/init-intake.sh",
        check.asp_leaf
    );
    Ok(())
}
