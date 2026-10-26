//! Initialises Shield's intake for the pool (`shield-api init-intake`, run
//! by `scripts/init-intake.sh` after every deploy and by `setup-testnet.sh`).
//!
//! A pool deposit from the intake needs two things the pool deploy does not
//! do: the intake's privacy keys in its SDK wallet, and its ASP leaf in the
//! allow-list. Without them every funded intent ends in `error`. Idempotent:
//! keys are derived only if missing and the leaf is inserted only if absent.

use std::time::Duration;

use crate::{
    adapters::pool_cli::CliPoolClient,
    asp::AspAction,
    domain::{ContractId, PoolField, TxHash},
    ports::{AspContract, AspKeys, ChainError, PoolError, TxStatus},
};

/// What [`init_intake`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntakeInit {
    /// Nothing to do: the leaf was already in the allow-list.
    AlreadyAllowListed { asp_leaf: PoolField },
    /// The operator inserted the leaf in this transaction.
    Inserted {
        asp_leaf: PoolField,
        tx_hash: TxHash,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum IntakeInitError {
    #[error("pool-cli failed ({}): {}", .0.code, .0.message)]
    Pool(PoolError),
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error("insert_leaf {0} failed on-chain: {1}")]
    Failed(TxHash, String),
    #[error("insert_leaf {0} was not confirmed in time; run again to check")]
    Unconfirmed(TxHash),
}

/// How [`init_intake`] waits for the `insert_leaf` transaction.
#[derive(Debug, Clone, Copy)]
pub struct Confirmation {
    pub poll: Duration,
    pub attempts: u32,
}

impl Default for Confirmation {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(2),
            attempts: 30,
        }
    }
}

/// Derives the intake's keys if missing and, if its leaf is not in the
/// allow-list `membership` (events from `from_ledger`), inserts it signed by
/// the operator (`asp`) and waits for the transaction.
pub async fn init_intake(
    pool: &CliPoolClient,
    asp: &dyn AspContract,
    membership: &ContractId,
    from_ledger: u32,
    confirmation: Confirmation,
) -> Result<IntakeInit, IntakeInitError> {
    let check = pool
        .check_intake(membership, from_ledger)
        .await
        .map_err(IntakeInitError::Pool)?;
    if check.allow_listed {
        return Ok(IntakeInit::AlreadyAllowListed {
            asp_leaf: check.asp_leaf,
        });
    }
    let keys = AspKeys {
        asp_leaf: check.asp_leaf.clone(),
        note_public_key: check.note_public_key,
    };
    let tx = asp.prepare(AspAction::Allow, &keys).await?;
    match asp.submit(&tx).await {
        // A timed-out submit may still land: the status below tells.
        Ok(()) | Err(ChainError::Unavailable(_)) => {}
        Err(err @ ChainError::Rejected(_)) => return Err(err.into()),
    }
    for attempt in 0..confirmation.attempts {
        if attempt > 0 {
            tokio::time::sleep(confirmation.poll).await;
        }
        match asp.status(&tx.hash).await {
            Ok(TxStatus::Success) => {
                return Ok(IntakeInit::Inserted {
                    asp_leaf: check.asp_leaf,
                    tx_hash: tx.hash,
                });
            }
            Ok(TxStatus::Failed(reason)) => return Err(IntakeInitError::Failed(tx.hash, reason)),
            Ok(TxStatus::Pending | TxStatus::NotFound) | Err(ChainError::Unavailable(_)) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Err(IntakeInitError::Unconfirmed(tx.hash))
}
