//! [`PoolClient`] over `apps/pool-cli`, run as a subprocess (the SPP SDK
//! cannot be linked next to `sqlx`). Protocol: `apps/pool-cli/README.md`.
//!
//! The child gets an empty environment plus only the variables it needs, so
//! no other API secret (admin token, operator seed, database URL) reaches it.
//! Only the auditor's instance gets the Global View Key ([`AUDITOR_ENV`]).

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use tokio::{process::Command, sync::Mutex};

use super::soroban::rpc::SorobanRpc;
use crate::{
    domain::{PoolField, TxHash},
    ports::{AuditedNote, AuditedTx, PoolAuditor, PoolClient, PoolError, PoolTx, TxStatus},
};

/// Variables forwarded to the CLI (same names as the API's `.env`).
pub const FORWARDED_ENV: [&str; 6] = [
    "SHIELD_INTAKE_SECRET",
    "SHIELD_DEPLOYMENT_FILE",
    "SHIELD_INTAKE_WALLET_PATH",
    "SHIELD_CIRCUITS_LOCK_FILE",
    "SHIELD_CIRCUITS_DIR",
    "SOROBAN_RPC_URL",
];

/// Extra variable of the auditor's instance: the Global View Key file.
pub const AUDITOR_ENV: &str = "SHIELD_GVK_KEY_FILE";

/// Proving takes ~10 s; the first run also downloads circuit artifacts.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone)]
pub struct CliPoolClient {
    binary: PathBuf,
    env: Vec<(String, String)>,
    timeout: Duration,
    rpc: SorobanRpc,
    intake: [u8; 32],
    /// Held while the CLI runs: every instance opens the same SDK wallet
    /// (SQLite), so runs that share a wallet must share this lock.
    wallet: Arc<Mutex<()>>,
}

impl CliPoolClient {
    /// `env` is read once here; values are only ever passed to the child.
    #[must_use]
    pub fn new(
        binary: PathBuf,
        env: Vec<(String, String)>,
        timeout: Duration,
        rpc: SorobanRpc,
        intake: [u8; 32],
    ) -> Self {
        Self {
            binary,
            env,
            timeout,
            rpc,
            intake,
            wallet: Arc::default(),
        }
    }

    /// Serialises this instance's runs with others holding the same lock.
    #[must_use]
    pub fn sharing_wallet(mut self, lock: Arc<Mutex<()>>) -> Self {
        self.wallet = lock;
        self
    }

    /// Collects [`FORWARDED_ENV`] from this process's environment.
    #[must_use]
    pub fn forwarded_env() -> Vec<(String, String)> {
        FORWARDED_ENV
            .iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .map(|value| ((*name).to_owned(), value))
            })
            .collect()
    }

    /// [`Self::forwarded_env`] plus the Global View Key, for [`PoolAuditor`].
    #[must_use]
    pub fn auditor_env() -> Vec<(String, String)> {
        let mut env = Self::forwarded_env();
        if let Ok(value) = std::env::var(AUDITOR_ENV) {
            env.push((AUDITOR_ENV.to_owned(), value));
        }
        env
    }

    async fn run(&self, args: &[&str]) -> Result<Value, PoolError> {
        let _wallet = self.wallet.lock().await;
        let child = Command::new(&self.binary)
            .args(args)
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                failure(
                    "cli_unavailable",
                    format!("cannot start pool-cli: {e}"),
                    false,
                )
            })?;
        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| failure("cli_timeout", "pool-cli timed out".into(), true))?
            .map_err(|e| failure("cli_unavailable", e.to_string(), true))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().last().unwrap_or_default();
        if output.status.success() {
            return serde_json::from_str(line).map_err(|e| {
                failure(
                    "cli_protocol",
                    format!("unexpected pool-cli output: {e}"),
                    true,
                )
            });
        }
        let err: CliFailure = serde_json::from_str(line).map_err(|_| {
            failure(
                "cli_protocol",
                format!("pool-cli failed ({}) without a JSON error", output.status),
                true,
            )
        })?;
        Err(PoolError {
            code: err.error,
            message: err.message,
            retryable: err.retryable,
        })
    }

    async fn prepare(&self, args: &[&str]) -> Result<PoolTx, PoolError> {
        let value = self.run(args).await?;
        let prepared: Prepared = serde_json::from_value(value).map_err(|e| {
            failure(
                "cli_protocol",
                format!("unexpected prepare output: {e}"),
                true,
            )
        })?;
        Ok(PoolTx {
            hash: TxHash::parse(&prepared.hash)
                .map_err(|e| failure("cli_protocol", e.to_string(), true))?,
            envelope_xdr: prepared.envelope_xdr,
            sequence: prepared.sequence,
        })
    }
}

#[async_trait]
impl PoolClient for CliPoolClient {
    async fn prepare_deposit(&self, amount: i64) -> Result<PoolTx, PoolError> {
        self.prepare(&["prepare-deposit", "--amount", &amount.to_string()])
            .await
    }

    async fn prepare_transfer(
        &self,
        amount: i64,
        note_public_key: &PoolField,
        encryption_public_key: &PoolField,
    ) -> Result<PoolTx, PoolError> {
        self.prepare(&[
            "prepare-transfer",
            "--amount",
            &amount.to_string(),
            "--note-key",
            note_public_key.as_str(),
            "--encryption-key",
            encryption_public_key.as_str(),
        ])
        .await
    }

    async fn submit(&self, envelope_xdr: &str) -> Result<(), PoolError> {
        self.run(&["submit", "--envelope", envelope_xdr])
            .await
            .map(|_| ())
    }

    async fn status(&self, hash: &TxHash) -> Result<TxStatus, PoolError> {
        let value = self.run(&["status", "--hash", hash.as_str()]).await?;
        let status: Status = serde_json::from_value(value).map_err(|e| {
            failure(
                "cli_protocol",
                format!("unexpected status output: {e}"),
                true,
            )
        })?;
        Ok(match status.status.as_str() {
            "success" => TxStatus::Success,
            "failed" => TxStatus::Failed(status.result_xdr.unwrap_or_default()),
            "not_found" => TxStatus::NotFound,
            _ => TxStatus::Pending,
        })
    }

    async fn intake_sequence(&self) -> Result<i64, PoolError> {
        self.rpc
            .account_sequence(&self.intake)
            .await
            .map_err(|e| failure("chain", e.to_string(), true))
    }
}

#[async_trait]
impl PoolAuditor for CliPoolClient {
    async fn audit(&self, limit: usize) -> Result<Vec<AuditedTx>, PoolError> {
        let value = self.run(&["audit", "--limit", &limit.to_string()]).await?;
        let audit: Audit = serde_json::from_value(value).map_err(|e| {
            failure(
                "cli_protocol",
                format!("unexpected audit output: {e}"),
                true,
            )
        })?;
        audit
            .transactions
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }
}

fn failure(code: &str, message: String, retryable: bool) -> PoolError {
    PoolError {
        code: code.to_owned(),
        message,
        retryable,
    }
}

#[derive(Deserialize)]
struct CliFailure {
    error: String,
    message: String,
    retryable: bool,
}

#[derive(Deserialize)]
struct Prepared {
    hash: String,
    envelope_xdr: String,
    sequence: i64,
}

#[derive(Deserialize)]
struct Status {
    status: String,
    result_xdr: Option<String>,
}

#[derive(Deserialize)]
struct Audit {
    transactions: Vec<AuditTx>,
}

#[derive(Deserialize)]
struct AuditTx {
    ledger: u32,
    inputs: Vec<AuditNote>,
    outputs: Vec<AuditNote>,
}

#[derive(Deserialize)]
struct AuditNote {
    owner: String,
    amount: String,
}

impl TryFrom<AuditTx> for AuditedTx {
    type Error = PoolError;

    fn try_from(tx: AuditTx) -> Result<Self, Self::Error> {
        let notes = |notes: Vec<AuditNote>| -> Result<Vec<AuditedNote>, PoolError> {
            notes
                .into_iter()
                .map(|note| {
                    Ok(AuditedNote {
                        owner: PoolField::parse(&note.owner)
                            .map_err(|e| failure("cli_protocol", e.to_string(), true))?,
                        stroops: note.amount.parse().map_err(|_| {
                            failure(
                                "cli_protocol",
                                format!("bad note amount `{}`", note.amount),
                                true,
                            )
                        })?,
                    })
                })
                .collect()
        };
        Ok(Self {
            ledger: tx.ledger,
            inputs: notes(tx.inputs)?,
            outputs: notes(tx.outputs)?,
        })
    }
}
