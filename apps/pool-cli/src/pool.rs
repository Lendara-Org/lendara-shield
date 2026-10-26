//! Runs one [`Command`] against the pool with the SPP SDK, as the intake
//! account. Stateless between runs: the SDK's wallet (SQLite) holds the
//! intake's keys and notes.

use std::collections::VecDeque;

use serde_json::{Value, json};
use stellar_private_payments::{
    CircuitStore, Client, Error as SdkError, LocalProver, LocalSigner, LocalStorage, PrivatePool,
    ProverHandle, SignerHandle,
    chain::{LocalSigner as Keypair, RpcClient},
    circuit_lock,
    gvk::{GvkAudit, GvkTxAudit},
    types::{
        ContractConfig, EncryptionPublicKey, Field, NoteAmount, NoteOwnerAddress, NotePublicKey,
        SignedTransaction, SignerAddress, TransferRecipient,
    },
    zk::gvk::GvkAuditedNote,
};

use crate::command::{CliError, Command, describe, hex};

pub const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";
const DEFAULT_RPC_URL: &str = "https://soroban-testnet.stellar.org";

/// Environment read by the CLI (same names as the API's `.env`).
struct Env {
    secret: String,
    deployment_file: String,
    wallet_path: String,
    rpc_url: String,
}

impl Env {
    fn load() -> Result<Self, CliError> {
        Ok(Self {
            secret: require("SHIELD_INTAKE_SECRET")?,
            deployment_file: require("SHIELD_DEPLOYMENT_FILE")?,
            wallet_path: require("SHIELD_INTAKE_WALLET_PATH")?,
            rpc_url: std::env::var("SOROBAN_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.into()),
        })
    }
}

fn require(name: &str) -> Result<String, CliError> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| CliError::new("config", format!("{name} is not set"), false))
}

/// An open SDK session as the intake account.
struct Session {
    account: stellar_private_payments::Account,
    pool_id: String,
    address: String,
}

/// Runs `command` and returns its JSON result.
pub async fn run(command: Command) -> Result<Value, CliError> {
    if let Command::Status { hash } = &command {
        // Needs no wallet or keys: just the RPC.
        let rpc_url = std::env::var("SOROBAN_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.into());
        return status(&rpc_url, hash).await;
    }
    let needs_prover = matches!(
        command,
        Command::PrepareDeposit { .. } | Command::PrepareTransfer { .. }
    );
    let session = open(needs_prover).await?;
    execute(&session, command).await
}

async fn open(needs_prover: bool) -> Result<Session, CliError> {
    let env = Env::load()?;
    let config = load_config(&env.deployment_file)?;
    let pool_config = config
        .enabled_pools()
        .next()
        .ok_or_else(|| CliError::new("config", "deployment has no enabled pool", false))?
        .clone();
    let storage = LocalStorage::open(&env.wallet_path).map_err(|e| sdk(&e))?;
    let client = if needs_prover {
        let lock =
            circuit_lock(&read(&require("SHIELD_CIRCUITS_LOCK_FILE")?)?).map_err(|e| sdk(&e))?;
        let store = CircuitStore::open(require("SHIELD_CIRCUITS_DIR")?, lock);
        store.ensure().await.map_err(|e| sdk(&e))?;
        let stem = pool_config.circuit_stem();
        let artifacts = store.artifacts(&stem.to_string()).map_err(|e| sdk(&e))?;
        let prover = LocalProver::from_artifacts(&[(stem, artifacts)]).map_err(|e| sdk(&e))?;
        Client::init(
            &env.rpc_url,
            storage.into(),
            ProverHandle::from(prover),
            config,
            None,
        )
    } else {
        Client::init_readonly(&env.rpc_url, storage.into(), config, None)
    }
    .map_err(|e| sdk(&e))?;

    let address = Keypair::from_secret(&env.secret)
        .map_err(|_| {
            CliError::new(
                "config",
                "SHIELD_INTAKE_SECRET is not a valid secret seed",
                false,
            )
        })?
        .public_key()
        .to_string();
    let signer = LocalSigner::new(
        &env.secret,
        TESTNET_PASSPHRASE,
        SignerAddress::new(address.as_str()),
    )
    .map_err(|e| sdk(&e))?;
    let account = client
        .account(
            NoteOwnerAddress::new(address.as_str()),
            SignerHandle::from(signer),
        )
        .map_err(|e| sdk(&e))?;
    Ok(Session {
        account,
        pool_id: pool_config.pool_contract_id,
        address,
    })
}

async fn execute(session: &Session, command: Command) -> Result<Value, CliError> {
    let pool = || session.account.pool(&session.pool_id).map_err(|e| sdk(&e));
    match command {
        Command::Keys => {
            let (note, encryption) = session
                .account
                .derive_privacy_keys()
                .await
                .map_err(|e| sdk(&e))?;
            let leaf = session
                .account
                .derive_asp_user_leaf()
                .await
                .map_err(|e| sdk(&e))?;
            Ok(json!({
                "account": session.address,
                "note_public_key": format!("0x{}", hex(&note.0)),
                "encryption_public_key": format!("0x{}", hex(&encryption.0)),
                "asp_leaf": leaf.to_string(),
            }))
        }
        Command::Balance => {
            let balance = pool()?.balance().await.map_err(|e| sdk(&e))?;
            Ok(json!({ "stroops": u128::from(balance).to_string() }))
        }
        Command::PrepareDeposit { amount } => {
            let pool = pool()?;
            let plan = pool
                .prepare_deposit(NoteAmount::from(amount))
                .map_err(|e| sdk(&e))?;
            prepare(&pool, plan).await
        }
        Command::PrepareTransfer {
            amount,
            note_key,
            encryption_key,
        } => {
            let pool = pool()?;
            let recipient = TransferRecipient::keys(
                NotePublicKey::parse(&note_key).map_err(|e| CliError::usage(&e.to_string()))?,
                EncryptionPublicKey::parse(&encryption_key)
                    .map_err(|e| CliError::usage(&e.to_string()))?,
            );
            let notes = pool.spendable_notes().await.map_err(|e| sdk(&e))?;
            let plan = pool
                .prepare_transfer(&notes, recipient, NoteAmount::from(amount))
                .await
                .map_err(|e| sdk(&e))?;
            prepare(&pool, plan).await
        }
        Command::Submit { envelope } => {
            let expected = describe(&envelope, TESTNET_PASSPHRASE)?.hash;
            let hash = pool()?
                .submit(SignedTransaction {
                    signed_xdr: envelope,
                })
                .await
                .map_err(|e| sdk(&e))?;
            if hash != expected {
                return Err(CliError::new(
                    "chain",
                    "the network reported another hash",
                    true,
                ));
            }
            Ok(json!({ "hash": hash }))
        }
        Command::Audit { limit } => audit(session, limit).await,
        Command::Status { .. } => Err(CliError::new(
            "internal",
            "status is handled before opening the wallet",
            false,
        )),
    }
}

/// The newest `limit` pool transactions decrypted with the Global View Key,
/// newest first. Only notes with value are listed (zero-value notes pad
/// every transaction). Owners are note public keys as the SDK prints them.
async fn audit(session: &Session, limit: usize) -> Result<Value, CliError> {
    let gvk = gvk_private_key()?;
    session.account.sync().await.map_err(|e| sdk(&e))?;
    let storage = session.account.storage().fork().map_err(|e| sdk(&e))?;
    let mut audit = GvkAudit::new(storage, &session.pool_id, gvk);
    let mut newest = VecDeque::with_capacity(limit + 1);
    while let Some(tx) = audit
        .next_tx()
        .await
        .map_err(|e| CliError::new("sdk", e.to_string(), true))?
    {
        newest.push_front(audited(&tx));
        newest.truncate(limit);
    }
    Ok(json!({ "transactions": newest }))
}

fn audited(tx: &GvkTxAudit) -> Value {
    let notes = |notes: Vec<Option<&GvkAuditedNote>>| -> Vec<Value> {
        notes
            .into_iter()
            .flatten()
            .filter_map(|n| {
                let amount = u128::from(n.note.amount().ok()?);
                (amount > 0).then(
                    || json!({ "owner": n.note.pk.to_string(), "amount": amount.to_string() }),
                )
            })
            .collect()
    };
    json!({
        "ledger": tx.ledger,
        "inputs": notes(tx.inputs.iter().map(|i| i.note.as_ref()).collect()),
        "outputs": notes(tx.outputs.iter().map(|o| o.note.as_ref()).collect()),
    })
}

/// The auditor's private key, from `SHIELD_GVK_KEY_FILE` (SPP's
/// `tools/gvkey-gen` JSON). Never printed, not even in errors.
fn gvk_private_key() -> Result<Field, CliError> {
    let file: Value = serde_json::from_str(&read(&require("SHIELD_GVK_KEY_FILE")?)?)
        .map_err(|_| CliError::new("config", "the GVK key file is not JSON", false))?;
    file["privateKey"]
        .as_str()
        .and_then(|raw| raw.parse::<Field>().ok())
        .ok_or_else(|| CliError::new("config", "the GVK key file has no valid privateKey", false))
}

/// Proves, simulates and signs a single-transaction plan.
async fn prepare(
    pool: &PrivatePool,
    mut plan: stellar_private_payments::plan::PreparedTransactionPlan,
) -> Result<Value, CliError> {
    if plan.tx_count() != 1 {
        return Err(CliError::new(
            "multi_transaction_plan",
            format!(
                "the operation needs {} transactions; only single-transaction plans are supported",
                plan.tx_count()
            ),
            false,
        ));
    }
    let mut prepared = pool.prove_next(&mut plan).await.map_err(|e| sdk(&e))?;
    pool.simulate(&mut prepared).await.map_err(|e| sdk(&e))?;
    let signed = pool.sign(&prepared).await.map_err(|e| sdk(&e))?;
    let described = describe(&signed.signed_xdr, TESTNET_PASSPHRASE)?;
    serde_json::to_value(described).map_err(|e| CliError::new("internal", e.to_string(), false))
}

async fn status(rpc_url: &str, hash: &str) -> Result<Value, CliError> {
    let rpc = RpcClient::new(rpc_url).map_err(|e| CliError::new("chain", e.to_string(), true))?;
    let response = rpc
        .get_transaction(hash)
        .await
        .map_err(|e| CliError::new("chain", e.to_string(), true))?;
    let status = match response.status.as_str() {
        "SUCCESS" => "success",
        "FAILED" => "failed",
        "NOT_FOUND" => "not_found",
        _ => "pending",
    };
    Ok(json!({ "status": status, "result_xdr": response.result_xdr }))
}

fn load_config(path: &str) -> Result<ContractConfig, CliError> {
    let config: ContractConfig = serde_json::from_str(&read(path)?)
        .map_err(|e| CliError::new("config", format!("invalid deployment file: {e}"), false))?;
    if config.network != "testnet" {
        return Err(CliError::new(
            "config",
            "only testnet deployments are supported",
            false,
        ));
    }
    Ok(config)
}

fn read(path: &str) -> Result<String, CliError> {
    std::fs::read_to_string(path)
        .map_err(|e| CliError::new("config", format!("cannot read {path}: {e}"), false))
}

/// Classifies SDK errors: network problems may succeed on retry; anything
/// about keys, membership or the wallet will not.
fn sdk(err: &SdkError) -> CliError {
    let message = err.to_string();
    match err {
        SdkError::Rpc(_) => CliError::new("chain", message, true),
        SdkError::RetentionGap(_) => CliError::new("retention_gap", message, false),
        SdkError::MembershipSync(_) => CliError::new("not_allowed", message, false),
        SdkError::AccountNotFound { .. } | SdkError::InvalidConfig(_) => {
            CliError::new("config", message, false)
        }
        SdkError::Plan(_) | SdkError::SpendSession(_) => CliError::new("plan", message, false),
        // Simulation failures land here (e.g. a proof built against an ASP
        // root that changed): preparing again is safe since nothing was sent.
        _ => CliError::new("sdk", message, true),
    }
}
