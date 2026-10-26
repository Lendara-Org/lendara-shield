//! Ports: what the domain needs from the outside world. Adapters implement
//! them against real services; tests use the in-memory fakes in
//! [`crate::testing`].

use std::fmt::Debug;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    asp::AspAction,
    domain::{PoolField, TxHash},
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
        let field = PoolField::parse(&format!("0x{}", "ab".repeat(32)))?;
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

    #[test]
    fn system_clock_follows_the_wall_clock() {
        let before = Utc::now();
        let now = SystemClock.now();
        assert!(now >= before && now <= Utc::now());
    }
}
