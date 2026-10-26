//! `intake_payments` and `watcher_cursors`.

use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

use super::StoreError;
use crate::ports::IncomingPayment;

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
