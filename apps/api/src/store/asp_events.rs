//! `asp_events`: every on-chain ASP update, written before it is sent.

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, prelude::FromRow};
use uuid::Uuid;

use super::{StoreError, corrupt, unique_violation};
use crate::{asp::AspAction, domain::TxHash};

const TABLE: &str = "asp_events";

/// An ASP update whose transaction may still land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedEvent {
    pub id: Uuid,
    pub action: AspAction,
    pub tx_hash: TxHash,
    pub valid_until: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    /// Another update for this user is already in flight.
    #[error("an ASP update is already in flight for this user")]
    InFlight,
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(FromRow)]
struct Row {
    id: Uuid,
    action: String,
    tx_hash: String,
    valid_until: DateTime<Utc>,
}

impl TryFrom<Row> for SubmittedEvent {
    type Error = StoreError;
    fn try_from(row: Row) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            action: row.action.parse().map_err(|e| corrupt(TABLE, e))?,
            tx_hash: TxHash::parse(&row.tx_hash).map_err(|e| corrupt(TABLE, e))?,
            valid_until: row.valid_until,
        })
    }
}

/// Records a transaction as submitted **before** it is sent.
pub async fn record_submitted(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    action: AspAction,
    tx_hash: &TxHash,
    valid_until: DateTime<Utc>,
) -> Result<SubmittedEvent, RecordError> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO asp_events (id, user_id, action, status, tx_hash, valid_until) \
         VALUES ($1, $2, $3, 'submitted', $4, $5)",
    )
    .bind(id)
    .bind(user_id)
    .bind(action.as_str())
    .bind(tx_hash.as_str())
    .bind(valid_until)
    .execute(db)
    .await
    .map_err(|err| match unique_violation(&err) {
        Some("asp_events_one_submitted_per_user") => RecordError::InFlight,
        _ => RecordError::Store(err.into()),
    })?;
    Ok(SubmittedEvent {
        id,
        action,
        tx_hash: tx_hash.clone(),
        valid_until,
    })
}

/// The user's in-flight update, if any.
pub async fn find_submitted(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Option<SubmittedEvent>, StoreError> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, action, tx_hash, valid_until FROM asp_events \
         WHERE user_id = $1 AND status = 'submitted'",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    row.map(TryInto::try_into).transpose()
}

pub async fn mark_confirmed(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE asp_events SET status = 'confirmed' WHERE id = $1 AND status = 'submitted'",
    )
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn mark_failed(db: impl PgExecutor<'_>, id: Uuid, error: &str) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE asp_events SET status = 'failed', error = $2 WHERE id = $1 AND status = 'submitted'",
    )
    .bind(id)
    .bind(error)
    .execute(db)
    .await?;
    Ok(())
}

/// Actions confirmed on-chain for the user, oldest first.
pub async fn confirmed_actions(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<AspAction>, StoreError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT action FROM asp_events WHERE user_id = $1 AND status = 'confirmed' \
         ORDER BY created_at, id",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|(action,)| action.parse().map_err(|e| corrupt(TABLE, e)))
        .collect()
}

/// Hash of the user's latest submitted or confirmed update.
pub async fn last_tx_hash(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Option<TxHash>, StoreError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT tx_hash FROM asp_events WHERE user_id = $1 AND status <> 'failed' \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    row.map(|(hash,)| TxHash::parse(&hash).map_err(|e| corrupt(TABLE, e)))
        .transpose()
}
