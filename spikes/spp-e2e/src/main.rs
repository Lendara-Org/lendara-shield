//! H1 spike: drive a Stellar Private Payments pool from a server process.
//!
//! Proves that the native Rust SDK can do, without a browser or Freighter:
//! derive pool keys for a server-held account, register them, deposit,
//! transfer privately to another registered `G...` address, withdraw, and
//! audit the pool with the Global View Key.
//!
//! This is throwaway spike code (see `docs/spikes/spp.md`); the API in H2+
//! wraps the same SDK calls behind ports and adapters.
//!
//! Usage: `spp-e2e <onboard | balance | deposit AMOUNT | transfer G... AMOUNT |
//! withdraw AMOUNT | audit>`. Amounts are in stroops.
//!
//! Environment:
//! - `SPIKE_SECRET`: secret key of the account that signs (testnet only).
//! - `SPIKE_WALLET`: path of the SDK's SQLite wallet for that account.
//! - `SPIKE_DEPLOYMENT`: path of the pool's `deployments.json`.
//! - `SPIKE_CIRCUITS_JSON`: trusted circuit lockfile matching the deployment.
//! - `SPIKE_CIRCUITS_DIR`: where circuit artifacts are cached.
//! - `SPIKE_GVK_KEY_FILE`: Global View Key private keypair (only for `audit`).
//! - `SPIKE_RPC_URL`: optional, defaults to the public testnet RPC.

use std::{env, error::Error, fs, process::ExitCode};

use stellar_private_payments::{
    CircuitStore, LocalProver, LocalSigner, LocalStorage, ProverHandle, SignerHandle,
    blocking::{Account, Client, PrivatePool},
    chain::LocalSigner as StellarKeypair,
    circuit_lock,
    gvk::GvkAudit,
    types::{ContractConfig, Field, NoteAmount, NoteOwnerAddress, PoolConfigEntry, SignerAddress},
    zk::gvk::GvkAuditedNote,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";
const DEFAULT_RPC_URL: &str = "https://soroban-testnet.stellar.org";
const EXPLORER: &str = "https://stellar.expert/explorer/testnet/tx";

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<()> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let session = Session::open()?;
    match args.as_slice() {
        ["onboard"] => session.onboard(),
        ["balance"] => session.balance(),
        ["deposit", amount] => session.deposit(parse_amount(amount)?),
        ["transfer", to, amount] => session.transfer(to, parse_amount(amount)?),
        ["withdraw", amount] => session.withdraw(parse_amount(amount)?),
        ["audit"] => session.audit(),
        _ => Err("usage: spp-e2e <onboard | balance | deposit AMOUNT | transfer G... AMOUNT | withdraw AMOUNT | audit>".into()),
    }
}

struct Session {
    client: Client,
    account: Account,
    pool: PrivatePool,
    pool_config: PoolConfigEntry,
}

impl Session {
    fn open() -> Result<Self> {
        let secret = require_env("SPIKE_SECRET")?;
        let config: ContractConfig =
            serde_json::from_str(&fs::read_to_string(require_env("SPIKE_DEPLOYMENT")?)?)?;
        let pool_config = config
            .enabled_pools()
            .next()
            .ok_or("deployment has no enabled pool")?
            .clone();

        let lock = circuit_lock(&fs::read_to_string(require_env("SPIKE_CIRCUITS_JSON")?)?)?;
        let store = CircuitStore::open(require_env("SPIKE_CIRCUITS_DIR")?, lock);
        store.ensure_blocking()?;
        let stem = pool_config.circuit_stem();
        let artifacts = store.artifacts(&stem.to_string())?;
        let prover = ProverHandle::from(LocalProver::from_artifacts(&[(stem, artifacts)])?);

        let storage = LocalStorage::open(&require_env("SPIKE_WALLET")?)?;
        let rpc_url = env::var("SPIKE_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_owned());
        // A pool deployed inside the RPC's 7-day window needs no bootnode.
        let client = Client::init(&rpc_url, storage, prover, config, None)?;

        let address = StellarKeypair::from_secret(&secret)?
            .public_key()
            .to_string();
        let signer = SignerHandle::from(LocalSigner::new(
            &secret,
            TESTNET_PASSPHRASE,
            SignerAddress::new(address.as_str()),
        )?);
        let account = client.account(NoteOwnerAddress::new(address.as_str()), signer)?;
        let pool = account.pool(&pool_config.pool_contract_id)?;

        println!("account: {address}");
        println!("pool:    {}", pool_config.pool_contract_id);
        Ok(Self {
            client,
            account,
            pool,
            pool_config,
        })
    }

    /// Derives pool keys, registers them, and prints the ASP membership leaf
    /// the ASP admin must insert before this account can transact.
    fn onboard(&self) -> Result<()> {
        let (note_key, encryption_key) = self.account.derive_privacy_keys()?;
        println!("note public key:       {note_key:?}");
        println!("encryption public key: {encryption_key:?}");
        if self.account.is_registered()? {
            println!("registry: already registered");
        } else {
            let result = self.account.register_public_keys()?;
            print_tx("register", &result.tx_hash);
        }
        println!("asp leaf: {}", self.account.derive_asp_user_leaf()?);
        Ok(())
    }

    fn balance(&self) -> Result<()> {
        self.account.sync()?;
        println!(
            "private balance: {} stroops",
            u128::from(self.pool.balance()?)
        );
        Ok(())
    }

    fn deposit(&self, amount: NoteAmount) -> Result<()> {
        let result = self.pool.deposit(amount)?;
        print_tx("deposit", &result.tx_hash);
        Ok(())
    }

    fn transfer(&self, to: &str, amount: NoteAmount) -> Result<()> {
        for result in self.pool.transfer(to.to_owned(), amount)? {
            print_tx("transfer", &result.tx_hash);
        }
        Ok(())
    }

    fn withdraw(&self, amount: NoteAmount) -> Result<()> {
        let to = self.account.user_address().to_string();
        for result in self.pool.withdraw(amount, to)? {
            print_tx("withdraw", &result.tx_hash);
        }
        Ok(())
    }

    /// Decrypts every pool transaction with the Global View Key.
    fn audit(&self) -> Result<()> {
        let keypair: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(require_env("SPIKE_GVK_KEY_FILE")?)?)?;
        let d_priv: Field = keypair["privateKey"]
            .as_str()
            .ok_or("GVK key file has no privateKey")?
            .parse()?;

        self.client.sync()?;
        let storage = self.client.storage().fork()?;
        let pool_id = self.pool_config.pool_contract_id.clone();
        // `GvkAudit` is async-only; the blocking client has no wrapper for it.
        tokio::runtime::Runtime::new()?.block_on(async {
            let mut audit = GvkAudit::new(storage, &pool_id, d_priv);
            while let Some(tx) = audit.next_tx().await? {
                println!(
                    "ledger {}: {} outputs, {} inputs",
                    tx.ledger,
                    tx.outputs.len(),
                    tx.inputs.len()
                );
                for out in &tx.outputs {
                    println!("  output: {}", describe(out.note.as_ref()));
                }
                for input in &tx.inputs {
                    println!("  input:  {}", describe(input.note.as_ref()));
                }
            }
            Ok(())
        })
    }
}

/// Owner key and amount of a note decrypted with the Global View Key.
fn describe(note: Option<&GvkAuditedNote>) -> String {
    match note.and_then(|n| n.note.amount().ok().map(|amount| (n, amount))) {
        Some((n, amount)) => format!(
            "owner_pk={} amount={} stroops",
            n.note.pk,
            u128::from(amount)
        ),
        None => "undecryptable".to_owned(),
    }
}

fn require_env(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("{name} is required").into())
}

fn parse_amount(raw: &str) -> Result<NoteAmount> {
    let stroops: u128 = raw.parse()?;
    if stroops == 0 {
        return Err("amount must be greater than zero".into());
    }
    Ok(NoteAmount::from(stroops))
}

fn print_tx(label: &str, hash: &str) {
    println!("{label}: {hash}\n  {EXPLORER}/{hash}");
}
