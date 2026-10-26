//! Against the real Lendara sandbox anchor (`make test-testnet`; needs
//! `.env`). Runs SEP-1 → SEP-10 → SEP-24 and opens a deposit that is never
//! completed (it expires in the sandbox).

use std::sync::Arc;

use secrecy::SecretString;
use shield_api::{
    adapters::anchor::{Sep24AnchorClient, Sep24AnchorConfig},
    domain::AccountId,
    ports::{AnchorClient, StartDeposit, SystemClock},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is not set; see .env.example"))
}

#[tokio::test]
#[ignore = "talks to the Lendara sandbox on testnet; run with `make test-testnet`"]
async fn testnet_sandbox_starts_an_interactive_deposit() -> TestResult {
    let client = Sep24AnchorClient::new(
        Sep24AnchorConfig {
            home_domain: env("RAMP_ANCHOR_HOME_DOMAIN")?,
            intake_secret: SecretString::from(env("SHIELD_INTAKE_SECRET")?),
            asset_issuer: AccountId::parse(&env("RAMP_ASSET_ISSUER")?)?,
            network_passphrase: "Test SDF Network ; September 2015".into(),
        },
        Arc::new(SystemClock),
    )?;

    let deposit = client
        .start_deposit(&StartDeposit {
            asset_code: env("RAMP_ASSET_CODE").unwrap_or_else(|_| "USDC".into()),
            source_asset: "iso4217:CRC".into(),
            customer_memo: 1,
            deposit_memo: 1,
        })
        .await?;

    assert!(!deposit.id.is_empty());
    assert!(deposit.interactive_url.starts_with("https://"));
    Ok(())
}
