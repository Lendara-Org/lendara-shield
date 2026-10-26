//! Follows deposits at the anchor (SEP-24 `GET /transaction`) while the
//! intent waits for the payment: the anchor is the only one who knows that
//! the user never paid or that KYC failed. The sandbox has no webhooks.
//!
//! It is also what funds an intent: when the anchor reports `completed`
//! with its Stellar transaction, the payment the watcher recorded in that
//! transaction (with the intent's memo) moves the intent to
//! `funds_received`. A payment the anchor does not claim never funds
//! anything, so dust sent with someone's memo cannot hijack their deposit.

use std::{sync::Arc, time::Duration};

use sqlx::PgPool;

use super::state::IntentStatus;
use crate::{
    domain::TxHash,
    ports::{AnchorClient, AnchorStatus, Clock},
    store::{
        StoreError, intake_payments,
        intents::{self, Changes, Intent, TransitionError},
    },
};

/// Time between two status checks of the same intent.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(20);
/// Wait when the anchor reports a payment Shield has not read on-chain yet.
pub const CONFIRM_RETRY: Duration = Duration::from_secs(5);
const BATCH: i64 = 20;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

impl From<sqlx::Error> for SyncError {
    fn from(err: sqlx::Error) -> Self {
        Self::Store(err.into())
    }
}

#[derive(Debug, Clone)]
pub struct AnchorSync {
    db: PgPool,
    anchor: Arc<dyn AnchorClient>,
    clock: Arc<dyn Clock>,
}

impl AnchorSync {
    #[must_use]
    pub fn new(db: PgPool, anchor: Arc<dyn AnchorClient>, clock: Arc<dyn Clock>) -> Self {
        Self { db, anchor, clock }
    }

    /// Checks every due intent once. Returns how many left `awaiting_anchor`
    /// (funded, expired or failed).
    pub async fn check_due(&self) -> Result<usize, SyncError> {
        let now = self.clock.now();
        let next = now + chrono::Duration::from_std(CHECK_INTERVAL).unwrap_or_default();
        let mut finished = 0;
        for (intent, customer_memo) in intents::due_awaiting_anchor(&self.db, now, BATCH).await? {
            let Some(anchor_tx_id) = intent.anchor_tx_id.as_deref() else {
                continue;
            };
            let (to, message) = match self
                .anchor
                .deposit_status(customer_memo, anchor_tx_id)
                .await
            {
                Ok(AnchorStatus::Expired) => (
                    IntentStatus::Expired,
                    "The anchor reports that the payment was not received.".to_owned(),
                ),
                Ok(AnchorStatus::Failed(reason)) => {
                    (IntentStatus::Error, format!("Anchor: {reason}"))
                }
                Ok(AnchorStatus::Completed(Some(hash))) => {
                    if self.fund(&intent, &hash).await? {
                        finished += 1;
                    } else {
                        let retry =
                            now + chrono::Duration::from_std(CONFIRM_RETRY).unwrap_or_default();
                        intents::schedule(&self.db, intent.id, retry).await?;
                    }
                    continue;
                }
                Ok(AnchorStatus::Completed(None)) => {
                    tracing::warn!(intent_id = %intent.id, "anchor completed without a stellar_transaction_id; cannot confirm the payment");
                    intents::schedule(&self.db, intent.id, next).await?;
                    continue;
                }
                Ok(AnchorStatus::Pending) => {
                    intents::schedule(&self.db, intent.id, next).await?;
                    continue;
                }
                Err(err) => {
                    tracing::warn!(intent_id = %intent.id, error = %err, "anchor status unavailable");
                    intents::schedule(&self.db, intent.id, next).await?;
                    continue;
                }
            };
            let mut conn = self.db.acquire().await?;
            match intents::transition(
                &mut conn,
                intent.id,
                IntentStatus::AwaitingAnchor,
                to,
                Changes {
                    message: Some(message),
                    ..Changes::default()
                },
            )
            .await
            {
                Ok(_) => {
                    finished += 1;
                    tracing::info!(intent_id = %intent.id, status = %to, "anchor deposit ended");
                }
                // Funded meanwhile by another round.
                Err(TransitionError::Stale(_)) => {}
                Err(err) => return Err(err.into()),
            }
        }
        Ok(finished)
    }

    /// Funds `intent` with the payment the anchor reports (`hash`), if the
    /// watcher already recorded it. Returns whether the intent was funded.
    async fn fund(&self, intent: &Intent, hash: &TxHash) -> Result<bool, SyncError> {
        let mut tx = self.db.begin().await?;
        let Some((operation_id, amount)) =
            intake_payments::lock_confirmed(&mut tx, hash, intent.memo).await?
        else {
            return Ok(false);
        };
        match intents::transition(
            &mut tx,
            intent.id,
            IntentStatus::AwaitingAnchor,
            IntentStatus::FundsReceived,
            Changes {
                amount_asset: Some(amount),
                intake_payment_tx_hash: Some(hash.clone()),
                ..Changes::default()
            },
        )
        .await
        {
            Ok(_) => {
                intake_payments::attach(&mut tx, &operation_id, intent.id).await?;
                tx.commit().await?;
                tracing::info!(intent_id = %intent.id, tx_hash = %hash, "anchor payment confirmed");
                Ok(true)
            }
            Err(TransitionError::Stale(_)) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
}
