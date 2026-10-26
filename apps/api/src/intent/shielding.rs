//! The shielding worker: moves a funded intent into the pool and on to the
//! user, `funds_received → shielding → transferring → completed`.
//!
//! Money integrity (AGENTS.md §5.3), for both the deposit and the transfer:
//! 1. Prepare (prove + sign) without sending; the hash is known.
//! 2. Store the transaction in the same DB write as the status change.
//! 3. Send. Any outcome is fine: the stored hash decides.
//! 4. On later rounds ask the network about the stored hash:
//!    - success → next step;
//!    - pending → wait;
//!    - failed on-chain (sequence consumed) → prepare a new one;
//!    - unknown → if the intake's sequence has not reached it, resend the
//!      **same** envelope (it can never pay twice); if the network refuses
//!      it, prepare a new one. If the sequence passed it, the transaction
//!      either landed and is not indexed yet, or can never land: wait until
//!      it has stayed unknown for [`UNSEEN_GRACE`] before replacing it.
//!
//! A transaction is only ever replaced when the old one provably cannot
//! land, so an intent is deposited and transferred exactly once. The worker
//! runs as a single sequential task: the intake has one sequence number and
//! one SDK wallet.

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::state::IntentStatus;
use crate::{
    asp::AspStatus,
    ports::{Clock, PoolClient, PoolError, PoolTx, TxStatus},
    store::{
        StoreError,
        intents::{self, Changes, Intent, PoolStep, TransitionError, WorkItem},
    },
};

/// Failed attempts (prepare errors, replaced transactions) before giving up.
pub const MAX_ATTEMPTS: i32 = 5;
/// How long a transaction must stay unknown after its sequence passed
/// before it is considered dead (covers RPC indexing lag).
pub const UNSEEN_GRACE: Duration = Duration::from_secs(60);
/// Time until a sent transaction is checked.
const CHECK_AFTER: Duration = Duration::from_secs(4);
/// First retry delay after a failure; doubles per attempt, capped.
const RETRY_BASE: Duration = Duration::from_secs(5);
const RETRY_CAP: Duration = Duration::from_secs(300);
const BATCH: i64 = 5;

/// Message of an intent stopped because its user's key is no longer allowed.
pub const BLOCKED_USER_MESSAGE: &str =
    "The account was blocked by the ASP; the funds are in Shield's custody and need review.";

#[derive(Debug, thiserror::Error)]
pub enum ShieldingError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

impl From<sqlx::Error> for ShieldingError {
    fn from(err: sqlx::Error) -> Self {
        Self::Store(err.into())
    }
}

#[derive(Debug, Clone)]
pub struct ShieldingWorker {
    db: PgPool,
    pool: Arc<dyn PoolClient>,
    clock: Arc<dyn Clock>,
}

impl ShieldingWorker {
    #[must_use]
    pub fn new(db: PgPool, pool: Arc<dyn PoolClient>, clock: Arc<dyn Clock>) -> Self {
        Self { db, pool, clock }
    }

    /// Advances every due intent by one step. Returns how many were touched.
    pub async fn process_due(&self) -> Result<usize, ShieldingError> {
        let items = intents::due_for_shielding(&self.db, self.clock.now(), BATCH).await?;
        for item in &items {
            match self.advance(item).await {
                // Another writer moved it first: nothing to do this round.
                Ok(()) | Err(ShieldingError::Transition(TransitionError::Stale(_))) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(items.len())
    }

    async fn advance(&self, item: &WorkItem) -> Result<(), ShieldingError> {
        let intent = &item.intent;
        if item.attempts >= MAX_ATTEMPTS {
            return self.give_up(intent).await;
        }
        match intent.status {
            // Nothing has moved yet: a blocked key gets no deposit.
            IntentStatus::FundsReceived if item.asp_status != AspStatus::Allowed => {
                self.stop_for_blocked_user(intent).await
            }
            IntentStatus::FundsReceived => self.start_deposit(item).await,
            IntentStatus::Shielding => self.follow(item, PoolStep::Deposit).await,
            IntentStatus::Transferring => self.follow(item, PoolStep::Transfer).await,
            _ => Ok(()),
        }
    }

    /// `funds_received`: prepare the deposit and store it with `shielding`.
    async fn start_deposit(&self, item: &WorkItem) -> Result<(), ShieldingError> {
        let tx = match self.pool.prepare_deposit(amount(&item.intent)).await {
            Ok(tx) => tx,
            Err(err) => return self.retry_later(item, &err).await,
        };
        let mut conn = self.db.acquire().await?;
        intents::transition(
            &mut conn,
            item.intent.id,
            IntentStatus::FundsReceived,
            IntentStatus::Shielding,
            Changes {
                pool_deposit: Some(tx.clone()),
                ..Changes::default()
            },
        )
        .await?;
        tracing::info!(intent_id = %item.intent.id, tx_hash = %tx.hash, "pool deposit prepared");
        self.send(item, &tx).await
    }

    /// `shielding` deposit confirmed: prepare the transfer, store it with
    /// `transferring`.
    async fn start_transfer(&self, item: &WorkItem) -> Result<(), ShieldingError> {
        let tx = match self
            .pool
            .prepare_transfer(
                amount(&item.intent),
                &item.note_public_key,
                &item.encryption_public_key,
            )
            .await
        {
            Ok(tx) => tx,
            Err(err) => return self.retry_later(item, &err).await,
        };
        let mut conn = self.db.acquire().await?;
        intents::transition(
            &mut conn,
            item.intent.id,
            IntentStatus::Shielding,
            IntentStatus::Transferring,
            Changes {
                pool_transfer: Some(tx.clone()),
                ..Changes::default()
            },
        )
        .await?;
        tracing::info!(intent_id = %item.intent.id, tx_hash = %tx.hash, "private transfer prepared");
        self.send(item, &tx).await
    }

    /// Follows the stored transaction of `step` (see the module docs).
    async fn follow(&self, item: &WorkItem, step: PoolStep) -> Result<(), ShieldingError> {
        let stored = match step {
            PoolStep::Deposit => item.intent.pool_deposit.as_ref(),
            PoolStep::Transfer => item.intent.pool_transfer.as_ref(),
        };
        // The DB constraints make this impossible; fail safe if it happens.
        let Some(tx) = stored else {
            return self.give_up(&item.intent).await;
        };
        let status = match self.pool.status(&tx.hash).await {
            Ok(status) => status,
            Err(err) => return self.check_later(item, &err).await,
        };
        if status != TxStatus::NotFound && item.unseen_since.is_some() {
            intents::set_unseen_since(&self.db, item.intent.id, None).await?;
        }
        match status {
            TxStatus::Success => match step {
                // The deposit landed in Shield's own note: keep it there
                // rather than send it to a key the ASP froze meanwhile.
                PoolStep::Deposit if item.asp_status != AspStatus::Allowed => {
                    self.stop_for_blocked_user(&item.intent).await
                }
                PoolStep::Deposit => self.start_transfer(item).await,
                PoolStep::Transfer => self.complete(&item.intent).await,
            },
            TxStatus::Pending => self.schedule(item.intent.id, CHECK_AFTER).await,
            TxStatus::Failed(reason) => {
                tracing::warn!(intent_id = %item.intent.id, tx_hash = %tx.hash, %reason, "pool transaction failed on-chain");
                self.replace(item, step).await
            }
            TxStatus::NotFound => {
                let sequence = match self.pool.intake_sequence().await {
                    Ok(sequence) => sequence,
                    Err(err) => return self.check_later(item, &err).await,
                };
                if sequence < tx.sequence {
                    return self.resend(item, step, tx).await;
                }
                // The sequence passed it: landed-but-not-indexed or dead.
                let now = self.clock.now();
                match item.unseen_since {
                    Some(since) if now - since >= grace() => {
                        intents::set_unseen_since(&self.db, item.intent.id, None).await?;
                        self.replace(item, step).await
                    }
                    Some(_) => self.schedule(item.intent.id, CHECK_AFTER).await,
                    None => {
                        tracing::warn!(intent_id = %item.intent.id, tx_hash = %tx.hash, "pool transaction unseen after its sequence; waiting");
                        intents::set_unseen_since(&self.db, item.intent.id, Some(now)).await?;
                        self.schedule(item.intent.id, CHECK_AFTER).await
                    }
                }
            }
        }
    }

    /// Sends the same envelope again (it can only land once).
    async fn resend(
        &self,
        item: &WorkItem,
        step: PoolStep,
        tx: &PoolTx,
    ) -> Result<(), ShieldingError> {
        match self.pool.submit(&tx.envelope_xdr).await {
            Ok(()) => self.schedule(item.intent.id, CHECK_AFTER).await,
            Err(err) if err.retryable => self.check_later(item, &err).await,
            // Refused outright (e.g. a proof against an ASP root that
            // changed): it is not pending anywhere, so replace it.
            Err(err) => {
                tracing::warn!(intent_id = %item.intent.id, code = %err.code, message = %err.message, "stored pool transaction refused");
                self.replace(item, step).await
            }
        }
    }

    /// Prepares a new transaction for `step` and stores it in place of the
    /// one that can no longer land. Counts as a failed attempt.
    async fn replace(&self, item: &WorkItem, step: PoolStep) -> Result<(), ShieldingError> {
        // The old transfer can no longer land: never build a new one for a
        // key the ASP froze meanwhile. The funds stay in Shield's note.
        if step == PoolStep::Transfer && item.asp_status != AspStatus::Allowed {
            return self.stop_for_blocked_user(&item.intent).await;
        }
        let id = item.intent.id;
        let prepared = match step {
            PoolStep::Deposit => self.pool.prepare_deposit(amount(&item.intent)).await,
            PoolStep::Transfer => {
                self.pool
                    .prepare_transfer(
                        amount(&item.intent),
                        &item.note_public_key,
                        &item.encryption_public_key,
                    )
                    .await
            }
        };
        let tx = match prepared {
            Ok(tx) => tx,
            Err(err) => return self.retry_later(item, &err).await,
        };
        let mut conn = self.db.acquire().await?;
        intents::replace_pool_tx(&mut conn, id, item.intent.status, step, &tx).await?;
        intents::record_attempt(&self.db, id, self.after(CHECK_AFTER)).await?;
        tracing::info!(intent_id = %id, tx_hash = %tx.hash, "pool transaction replaced");
        self.send(item, &tx).await
    }

    /// Sends a stored transaction. The outcome only decides when to look
    /// again: the next round asks the network about the hash.
    async fn send(&self, item: &WorkItem, tx: &PoolTx) -> Result<(), ShieldingError> {
        if let Err(err) = self.pool.submit(&tx.envelope_xdr).await {
            tracing::warn!(intent_id = %item.intent.id, tx_hash = %tx.hash, code = %err.code, message = %err.message, "pool submit outcome unknown");
        }
        self.schedule(item.intent.id, CHECK_AFTER).await
    }

    async fn complete(&self, intent: &Intent) -> Result<(), ShieldingError> {
        let mut conn = self.db.acquire().await?;
        intents::transition(
            &mut conn,
            intent.id,
            IntentStatus::Transferring,
            IntentStatus::Completed,
            Changes::default(),
        )
        .await?;
        tracing::info!(intent_id = %intent.id, "intent completed");
        Ok(())
    }

    /// The recipient's key is no longer allowed. The funds stay in Shield's
    /// custody (the intake, or Shield's note after the deposit) for review;
    /// a transfer already sent is never chased.
    async fn stop_for_blocked_user(&self, intent: &Intent) -> Result<(), ShieldingError> {
        let mut conn = self.db.acquire().await?;
        intents::transition(
            &mut conn,
            intent.id,
            intent.status,
            IntentStatus::Error,
            Changes {
                message: Some(BLOCKED_USER_MESSAGE.to_owned()),
                ..Changes::default()
            },
        )
        .await?;
        tracing::warn!(intent_id = %intent.id, "shielding stopped: the user's key is not allowed; manual review needed");
        Ok(())
    }

    async fn give_up(&self, intent: &Intent) -> Result<(), ShieldingError> {
        let mut conn = self.db.acquire().await?;
        intents::transition(
            &mut conn,
            intent.id,
            intent.status,
            IntentStatus::Error,
            Changes {
                message: Some(
                    "Shielding failed repeatedly; the funds are in Shield's custody and need review."
                        .to_owned(),
                ),
                ..Changes::default()
            },
        )
        .await?;
        tracing::error!(intent_id = %intent.id, "shielding gave up; manual review needed");
        Ok(())
    }

    /// A failed attempt: back off exponentially and count it.
    async fn retry_later(&self, item: &WorkItem, err: &PoolError) -> Result<(), ShieldingError> {
        tracing::warn!(intent_id = %item.intent.id, code = %err.code, message = %err.message, retryable = err.retryable, "shielding step failed");
        let exponent = u32::try_from(item.attempts).unwrap_or(0).min(16);
        let delay = RETRY_BASE
            .saturating_mul(2u32.saturating_pow(exponent))
            .min(RETRY_CAP);
        intents::record_attempt(&self.db, item.intent.id, self.after(delay)).await?;
        Ok(())
    }

    /// The network could not be asked: try again soon, not counted.
    async fn check_later(&self, item: &WorkItem, err: &PoolError) -> Result<(), ShieldingError> {
        tracing::warn!(intent_id = %item.intent.id, code = %err.code, message = %err.message, "pool status unknown");
        self.schedule(item.intent.id, CHECK_AFTER).await
    }

    async fn schedule(&self, id: uuid::Uuid, after: Duration) -> Result<(), ShieldingError> {
        intents::schedule(&self.db, id, self.after(after)).await?;
        Ok(())
    }

    fn after(&self, delay: Duration) -> DateTime<Utc> {
        self.clock.now() + chrono::Duration::from_std(delay).unwrap_or_default()
    }
}

fn grace() -> chrono::Duration {
    chrono::Duration::from_std(UNSEEN_GRACE).unwrap_or_default()
}

/// The whole amount received from the anchor, in stroops.
fn amount(intent: &Intent) -> i64 {
    intent
        .amount_asset
        .as_ref()
        .map_or(0, crate::domain::DecimalAmount::to_stroops)
}
