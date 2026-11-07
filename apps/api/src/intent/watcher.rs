//! Watches the intake account and records every incoming payment. A payment
//! with an intent's memo does **not** fund it on its own: anyone can send
//! the asset with a memo. It is recorded as unconfirmed and the intent is
//! checked with the anchor right away; [`super::anchor_sync`] funds the
//! intent only with the payment the anchor reports as its own.
//!
//! Each page is processed in one database transaction together with the
//! cursor, so a crash never skips a payment or applies one twice; payments
//! are also recorded by operation id, which makes re-reading a page a no-op.

use std::sync::Arc;

use sqlx::PgPool;

use super::matching::{self, ExpectedAsset};
use crate::{
    ports::{IncomingPayment, IntakeLedger, LedgerError},
    store::{
        StoreError, intake_payments,
        intents::{self, TransitionError},
    },
};

/// Name of the Horizon payments cursor in `watcher_cursors`.
pub const CURSOR: &str = "intake_payments";
const PAGE_SIZE: u32 = 100;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WatchReport {
    /// New payments recorded in this poll.
    pub recorded: usize,
    /// Of those, payments that carry the memo of an intent waiting for the
    /// anchor (funded once the anchor confirms them).
    pub matched: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<sqlx::Error> for WatchError {
    fn from(err: sqlx::Error) -> Self {
        Self::Store(err.into())
    }
}

#[derive(Debug, Clone)]
pub struct IntakeWatcher {
    db: PgPool,
    ledger: Arc<dyn IntakeLedger>,
    asset: ExpectedAsset,
}

impl IntakeWatcher {
    #[must_use]
    pub fn new(db: PgPool, ledger: Arc<dyn IntakeLedger>, asset: ExpectedAsset) -> Self {
        Self { db, ledger, asset }
    }

    /// Reads every new payment and applies it.
    pub async fn poll(&self) -> Result<WatchReport, WatchError> {
        let mut report = WatchReport::default();
        loop {
            let cursor = intake_payments::cursor(&self.db, CURSOR).await?;
            let page = self
                .ledger
                .payments_after(cursor.as_deref(), PAGE_SIZE)
                .await?;
            let Some(next_cursor) = page.next_cursor else {
                return Ok(report);
            };

            let mut tx = self.db.begin().await?;
            for payment in &page.payments {
                if let Some(matched) = self.apply(&mut tx, payment).await? {
                    report.recorded += 1;
                    report.matched += usize::from(matched);
                }
            }
            intake_payments::set_cursor(&mut tx, CURSOR, &next_cursor).await?;
            tx.commit().await?;
        }
    }

    /// Records one payment. Returns `None` if it was already recorded, else
    /// whether it matched an intent waiting for the anchor.
    async fn apply(
        &self,
        conn: &mut sqlx::PgConnection,
        payment: &IncomingPayment,
    ) -> Result<Option<bool>, WatchError> {
        if intake_payments::exists(&mut *conn, &payment.operation_id).await? {
            return Ok(None);
        }
        let intent = match payment.memo.and_then(|m| i64::try_from(m).ok()) {
            Some(memo) => intents::lock_by_memo(conn, memo).await?,
            None => None,
        };

        match matching::classify(payment, &self.asset, intent.as_ref().map(|i| i.status)) {
            Ok(()) => {
                let Some(intent) = intent else {
                    return Ok(Some(false));
                };
                intake_payments::record(conn, payment, None, Some(intake_payments::UNCONFIRMED))
                    .await?;
                intents::check_now(&mut *conn, intent.id).await?;
                tracing::info!(intent_id = %intent.id, tx_hash = %payment.tx_hash, "payment with an intent memo; asking the anchor");
                Ok(Some(true))
            }
            Err(unmatched) => {
                let reason = unmatched.reason();
                intake_payments::record(conn, payment, None, Some(&reason)).await?;
                tracing::warn!(tx_hash = %payment.tx_hash, %reason, "intake payment needs review");
                Ok(Some(false))
            }
        }
    }
}
