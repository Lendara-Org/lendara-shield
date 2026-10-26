//! Ports: what the domain needs from the outside world. Adapters implement
//! them against real services; tests use the in-memory fakes in
//! [`crate::testing`].

use std::fmt::Debug;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    asp::AspAction,
    domain::{AccountId, DecimalAmount, PoolField, TxHash},
};

/// The public values an ASP update writes on-chain for one user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AspKeys {
    /// Allow-list (membership tree) leaf.
    pub asp_leaf: PoolField,
    /// Block-list (non-membership tree) key: the note public key.
    pub note_public_key: PoolField,
}

/// A signed transaction ready to send. Its hash is known before sending, so
/// the caller can persist it first (CLAUDE.md §0.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTx {
    pub hash: TxHash,
    /// Base64 XDR envelope.
    pub envelope_xdr: String,
    /// Upper time bound: the network rejects the transaction after this.
    pub valid_until: DateTime<Utc>,
}

/// Network status of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxStatus {
    /// Seen by the network, not yet in a ledger.
    Pending,
    /// Included and successful.
    Success,
    /// Included or rejected with an error.
    Failed(String),
    /// Unknown to the network (never received, or dropped).
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    /// Temporary: network, RPC or timeout. Retrying may succeed, and a
    /// submitted transaction may still land.
    #[error("chain unavailable: {0}")]
    Unavailable(String),
    /// Definitive: the network refused the transaction; it will not land.
    #[error("transaction rejected: {0}")]
    Rejected(String),
}

/// Writes to the pool's ASP contracts, signed by the ASP admin (operator).
#[async_trait]
pub trait AspContract: Send + Sync + Debug {
    /// Builds, simulates and signs the transaction for `action`.
    async fn prepare(&self, action: AspAction, keys: &AspKeys) -> Result<PreparedTx, ChainError>;
    /// Sends a prepared transaction. Does not wait for inclusion.
    async fn submit(&self, tx: &PreparedTx) -> Result<(), ChainError>;
    /// Current network status of a transaction.
    async fn status(&self, hash: &TxHash) -> Result<TxStatus, ChainError>;
}

/// What Shield asks the anchor when a ramp intent starts (SEP-24 deposit to
/// Shield's intake account).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartDeposit {
    /// Asset the anchor pays out (`USDC`).
    pub asset_code: String,
    /// Off-chain asset the user pays with, SEP-38 format (`iso4217:CRC`).
    pub source_asset: String,
    /// The user's SEP-10 memo: each user is a distinct customer at the anchor.
    pub customer_memo: i64,
    /// The intent's SEP-24 memo: the anchor's payment to the intake carries
    /// it, which is how the watcher matches payments to intents.
    pub deposit_memo: i64,
}

/// A deposit the anchor started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorDeposit {
    /// The anchor's SEP-24 transaction id.
    pub id: String,
    /// Interactive URL to open for the user.
    pub interactive_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnchorError {
    /// Network, timeout, 5xx or an unexpected response.
    #[error("anchor unavailable: {0}")]
    Unavailable(String),
    /// The anchor answered but refused (4xx, failed SEP-10 validation).
    #[error("anchor refused: {0}")]
    Refused(String),
}

/// Where a SEP-24 deposit stands at the anchor, reduced to what Shield
/// acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorStatus {
    /// Waiting for the user or the anchor (`incomplete`, `pending_*`).
    Pending,
    /// The anchor sent the funds; the watcher sees the payment on-chain.
    Completed,
    /// The user never paid (`expired`).
    Expired,
    /// The anchor gave up (`error`, `refunded`, `no_market`, `too_small`,
    /// `too_large`), with the anchor's message if any.
    Failed(String),
}

/// An anchor spoken to only through SEP-1, SEP-10 and SEP-24.
#[async_trait]
pub trait AnchorClient: Send + Sync + Debug {
    /// SEP-1 home domain of the anchor.
    fn home_domain(&self) -> &str;
    /// Starts an interactive SEP-24 deposit to Shield's intake account.
    async fn start_deposit(&self, request: &StartDeposit) -> Result<AnchorDeposit, AnchorError>;
    /// Status of a deposit, authenticating as the user's customer memo.
    async fn deposit_status(
        &self,
        customer_memo: i64,
        anchor_tx_id: &str,
    ) -> Result<AnchorStatus, AnchorError>;
}

/// A payment received by the intake account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingPayment {
    /// Ledger operation id (unique per payment).
    pub operation_id: String,
    pub tx_hash: TxHash,
    pub from: AccountId,
    pub asset_code: String,
    pub asset_issuer: String,
    pub amount: DecimalAmount,
    /// `id` memo of the transaction, if it has one.
    pub memo: Option<u64>,
}

/// One page of the intake's operations, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaymentPage {
    /// Incoming payments only (outgoing and other operations are skipped).
    pub payments: Vec<IncomingPayment>,
    /// Resume point after this page (covers skipped operations too); `None`
    /// when the page is empty.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ledger unavailable: {0}")]
pub struct LedgerError(pub String);

/// Read access to the intake account's payments (Horizon).
#[async_trait]
pub trait IntakeLedger: Send + Sync + Debug {
    /// Operations after `cursor` (from the start when `None`), at most `limit`.
    async fn payments_after(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaymentPage, LedgerError>;
}

/// Placeholder used until the SEP-24 client is configured (H6a2): every
/// deposit fails as temporarily unavailable.
#[derive(Debug, Clone)]
pub struct UnconfiguredAnchorClient {
    pub home_domain: String,
}

#[async_trait]
impl AnchorClient for UnconfiguredAnchorClient {
    fn home_domain(&self) -> &str {
        &self.home_domain
    }
    async fn start_deposit(&self, _: &StartDeposit) -> Result<AnchorDeposit, AnchorError> {
        Err(AnchorError::Unavailable(
            "SEP-24 client not configured".into(),
        ))
    }
    async fn deposit_status(&self, _: i64, _: &str) -> Result<AnchorStatus, AnchorError> {
        Err(AnchorError::Unavailable(
            "SEP-24 client not configured".into(),
        ))
    }
}

/// Source of the current time, injected so tests control it.
pub trait Clock: Send + Sync + Debug {
    fn now(&self) -> DateTime<Utc>;
}

/// Wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Placeholder used until the Soroban adapter is configured (H5b): every
/// call fails as temporarily unavailable, so users stay `pending`.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnconfiguredAspContract;

#[async_trait]
impl AspContract for UnconfiguredAspContract {
    async fn prepare(&self, _: AspAction, _: &AspKeys) -> Result<PreparedTx, ChainError> {
        Err(ChainError::Unavailable(
            "ASP contract adapter not configured".into(),
        ))
    }
    async fn submit(&self, _: &PreparedTx) -> Result<(), ChainError> {
        Err(ChainError::Unavailable(
            "ASP contract adapter not configured".into(),
        ))
    }
    async fn status(&self, _: &TxHash) -> Result<TxStatus, ChainError> {
        Err(ChainError::Unavailable(
            "ASP contract adapter not configured".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Result<AspKeys, crate::domain::PoolFieldError> {
        let field = PoolField::parse(&format!("0x{}", "0a".repeat(32)))?;
        Ok(AspKeys {
            asp_leaf: field.clone(),
            note_public_key: field,
        })
    }

    #[tokio::test]
    async fn unconfigured_adapter_is_always_temporarily_unavailable()
    -> Result<(), Box<dyn std::error::Error>> {
        let chain = UnconfiguredAspContract;
        let hash = TxHash::from_bytes([1; 32]);
        let prepared = PreparedTx {
            hash: hash.clone(),
            envelope_xdr: String::new(),
            valid_until: Utc::now(),
        };
        assert!(matches!(
            chain.prepare(AspAction::Allow, &keys()?).await,
            Err(ChainError::Unavailable(_))
        ));
        assert!(matches!(
            chain.submit(&prepared).await,
            Err(ChainError::Unavailable(_))
        ));
        assert!(matches!(
            chain.status(&hash).await,
            Err(ChainError::Unavailable(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn unconfigured_anchor_is_unavailable() {
        let anchor = UnconfiguredAnchorClient {
            home_domain: "anchor.example".into(),
        };
        assert_eq!(anchor.home_domain(), "anchor.example");
        let request = StartDeposit {
            asset_code: "USDC".into(),
            source_asset: "iso4217:CRC".into(),
            customer_memo: 1,
            deposit_memo: 1,
        };
        assert!(matches!(
            anchor.start_deposit(&request).await,
            Err(AnchorError::Unavailable(_))
        ));
    }

    #[test]
    fn system_clock_follows_the_wall_clock() {
        let before = Utc::now();
        let now = SystemClock.now();
        assert!(now >= before && now <= Utc::now());
    }
}
