//! `intake_payments` and `watcher_cursors`.

use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

use super::{StoreError, corrupt};
use crate::{
    domain::{DecimalAmount, TxHash},
    ports::IncomingPayment,
};

/// `unmatched_reason` of a payment that carries an intent's memo but that
/// the anchor has not (or not yet) confirmed as its own.
pub const UNCONFIRMED: &str = "matches an intent's memo; not confirmed by the anchor";

/// Records a payment once (by operation id). Returns `false` if it was
/// already recorded, so re-reading a page never processes it twice.
pub async fn record(
    conn: &mut PgConnection,
    payment: &IncomingPayment,
    intent_id: Option<Uuid>,
    unmatched_reason: Option<&str>,
) -> Result<bool, StoreError> {
    let inserted = sqlx::query(
        "INSERT INTO intake_payments (operation_id, tx_hash, from_account, asset_code, \
         asset_issuer, amount, memo, intent_id, unmatched_reason) \
         VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9) \
         ON CONFLICT (operation_id) DO NOTHING",
    )
    .bind(&payment.operation_id)
    .bind(payment.tx_hash.as_str())
    .bind(payment.from.as_str())
    .bind(&payment.asset_code)
    .bind(&payment.asset_issuer)
    .bind(payment.amount.as_str())
    .bind(payment.memo.and_then(|m| i64::try_from(m).ok()))
    .bind(intent_id)
    .bind(unmatched_reason)
    .execute(conn)
    .await?
    .rows_affected();
    Ok(inserted == 1)
}

/// The unattached payment in transaction `tx_hash` with memo `memo`, locked
/// until the transaction ends. `None` if Shield has not seen it yet (or it
/// already funded something). A transaction with several such payments is
/// not one the anchor would send: it is treated as unseen and left for review.
pub async fn lock_confirmed(
    conn: &mut PgConnection,
    tx_hash: &TxHash,
    memo: i64,
) -> Result<Option<(String, DecimalAmount)>, StoreError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT operation_id, amount::text FROM intake_payments \
         WHERE tx_hash = $1 AND memo = $2 AND intent_id IS NULL FOR UPDATE",
    )
    .bind(tx_hash.as_str())
    .bind(memo)
    .fetch_all(conn)
    .await?;
    match rows.as_slice() {
        [(operation_id, amount)] => Ok(Some((
            operation_id.clone(),
            DecimalAmount::parse(amount).map_err(|e| corrupt("intake_payments", e))?,
        ))),
        _ => Ok(None),
    }
}

/// Marks a recorded payment as the one that funded `intent_id`.
pub async fn attach(
    conn: &mut PgConnection,
    operation_id: &str,
    intent_id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE intake_payments SET intent_id = $2, unmatched_reason = NULL WHERE operation_id = $1",
    )
    .bind(operation_id)
    .bind(intent_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether a payment was already recorded.
pub async fn exists(db: impl PgExecutor<'_>, operation_id: &str) -> Result<bool, StoreError> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM intake_payments WHERE operation_id = $1)")
            .bind(operation_id)
            .fetch_one(db)
            .await?,
    )
}

/// Payments that funded nothing: money in custody that needs review.
pub async fn unmatched_count(db: impl PgExecutor<'_>) -> Result<i64, StoreError> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM intake_payments WHERE intent_id IS NULL")
            .fetch_one(db)
            .await?,
    )
}

pub async fn cursor(db: impl PgExecutor<'_>, name: &str) -> Result<Option<String>, StoreError> {
    Ok(
        sqlx::query_scalar("SELECT cursor FROM watcher_cursors WHERE name = $1")
            .bind(name)
            .fetch_optional(db)
            .await?,
    )
}

pub async fn set_cursor(
    conn: &mut PgConnection,
    name: &str,
    cursor: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO watcher_cursors (name, cursor) VALUES ($1, $2) \
         ON CONFLICT (name) DO UPDATE SET cursor = EXCLUDED.cursor",
    )
    .bind(name)
    .bind(cursor)
    .execute(conn)
    .await?;
    Ok(())
}
