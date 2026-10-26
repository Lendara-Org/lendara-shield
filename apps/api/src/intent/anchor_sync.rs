//! Follows deposits at the anchor (SEP-24 `GET /transaction`) while the
//! intent waits for the payment: the anchor is the only one who knows that
//! the user never paid or that KYC failed. The sandbox has no webhooks.
//!
//! A payment seen on-chain always wins: the watcher and this sync both move
//! `awaiting_anchor` only if the intent is still in it.

use std::{sync::Arc, time::Duration};

use sqlx::PgPool;

use super::state::IntentStatus;
use crate::{
    ports::{AnchorClient, AnchorStatus, Clock},
    store::{
        StoreError,
        intents::{self, Changes, TransitionError},
    },
};

/// Time between two status checks of the same intent.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(20);
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

    /// Checks every due intent once. Returns how many reached a final state.
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
                Ok(AnchorStatus::Pending | AnchorStatus::Completed) => {
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
                // The payment arrived meanwhile: the watcher already moved it.
                Err(TransitionError::Stale(_)) => {}
                Err(err) => return Err(err.into()),
            }
        }
        Ok(finished)
    }
}
