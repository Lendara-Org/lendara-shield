//! `intents` and `intent_events`. The only place that writes an intent's
//! status: every change is checked by [`state::check`], applied only if the
//! row is still in the expected status (no lost updates), and recorded in
//! `intent_events` in the same transaction.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgExecutor, prelude::FromRow};
use uuid::Uuid;

use super::{StoreError, corrupt};
use crate::{
    domain::{DecimalAmount, FiatCurrency, TxHash},
    intent::state::{self, IntentStatus, InvalidTransition},
};

const TABLE: &str = "intents";

/// A ramp intent as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: IntentStatus,
    pub fiat_currency: FiatCurrency,
    pub amount_fiat: DecimalAmount,
    pub asset_code: String,
    pub amount_asset: Option<DecimalAmount>,
    /// SEP-24 memo (type `id`).
    pub memo: i64,
    pub anchor_home_domain: String,
    pub anchor_tx_id: Option<String>,
    pub interactive_url: Option<String>,
    pub intake_payment_tx_hash: Option<TxHash>,
    pub pool_deposit_tx_hash: Option<TxHash>,
    pub pool_transfer_tx_hash: Option<TxHash>,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewIntent {
    pub user_id: Uuid,
    pub fiat_currency: FiatCurrency,
    pub amount_fiat: DecimalAmount,
    pub asset_code: String,
    pub anchor_home_domain: String,
}

/// Fields a transition may set; `None` leaves the column unchanged.
#[derive(Debug, Clone, Default)]
pub struct Changes {
    pub anchor_tx_id: Option<String>,
    pub interactive_url: Option<String>,
    pub amount_asset: Option<DecimalAmount>,
    pub intake_payment_tx_hash: Option<TxHash>,
    pub pool_deposit_tx_hash: Option<TxHash>,
    pub pool_transfer_tx_hash: Option<TxHash>,
    pub message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error(transparent)]
    Invalid(#[from] InvalidTransition),
    /// Another writer changed the status first.
    #[error("intent {0} is no longer in the expected status")]
    Stale(Uuid),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<sqlx::Error> for TransitionError {
    fn from(err: sqlx::Error) -> Self {
        Self::Store(err.into())
    }
}

/// Column list as text (amounts cast so they never pass through floats).
macro_rules! columns {
    () => {
        "id, user_id, status, fiat_currency, amount_fiat::text AS amount_fiat, asset_code, \
         amount_asset::text AS amount_asset, memo, anchor_home_domain, anchor_tx_id, \
         interactive_url, intake_payment_tx_hash, pool_deposit_tx_hash, pool_transfer_tx_hash, \
         message, created_at, updated_at, completed_at"
    };
}

#[derive(FromRow)]
struct Row {
    id: Uuid,
    user_id: Uuid,
    status: String,
    fiat_currency: String,
    amount_fiat: String,
    asset_code: String,
    amount_asset: Option<String>,
    memo: i64,
    anchor_home_domain: String,
    anchor_tx_id: Option<String>,
    interactive_url: Option<String>,
    intake_payment_tx_hash: Option<String>,
    pool_deposit_tx_hash: Option<String>,
    pool_transfer_tx_hash: Option<String>,
    message: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

/// Postgres renders `numeric(20, 7)` with all 7 decimals; `DecimalAmount`
/// canonicalises them away.
fn amount(raw: &str) -> Result<DecimalAmount, StoreError> {
    DecimalAmount::parse(raw).map_err(|e| corrupt(TABLE, e))
}

fn hash(raw: Option<String>) -> Result<Option<TxHash>, StoreError> {
    raw.map(|h| TxHash::parse(&h).map_err(|e| corrupt(TABLE, e)))
        .transpose()
}

impl TryFrom<Row> for Intent {
    type Error = StoreError;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            user_id: row.user_id,
            status: row.status.parse().map_err(|e| corrupt(TABLE, e))?,
            fiat_currency: FiatCurrency::parse(&row.fiat_currency)
                .map_err(|e| corrupt(TABLE, e))?,
            amount_fiat: amount(&row.amount_fiat)?,
            asset_code: row.asset_code,
            amount_asset: row.amount_asset.as_deref().map(amount).transpose()?,
            memo: row.memo,
            anchor_home_domain: row.anchor_home_domain,
            anchor_tx_id: row.anchor_tx_id,
            interactive_url: row.interactive_url,
            intake_payment_tx_hash: hash(row.intake_payment_tx_hash)?,
            pool_deposit_tx_hash: hash(row.pool_deposit_tx_hash)?,
            pool_transfer_tx_hash: hash(row.pool_transfer_tx_hash)?,
            message: row.message,
            created_at: row.created_at,
            updated_at: row.updated_at,
            completed_at: row.completed_at,
        })
    }
}

/// Inserts an intent in `created` and records its first event.
pub async fn insert_created(
    conn: &mut PgConnection,
    new: &NewIntent,
) -> Result<Intent, StoreError> {
    let row: Row = sqlx::query_as(concat!(
        "INSERT INTO intents (id, user_id, mode, status, fiat_currency, amount_fiat, asset_code, \
         anchor_home_domain) VALUES ($1, $2, 'ramp', 'created', $3, $4::numeric, $5, $6) \
         RETURNING ",
        columns!()
    ))
    .bind(Uuid::now_v7())
    .bind(new.user_id)
    .bind(new.fiat_currency.as_str())
    .bind(new.amount_fiat.as_str())
    .bind(&new.asset_code)
    .bind(&new.anchor_home_domain)
    .fetch_one(&mut *conn)
    .await?;
    let intent: Intent = row.try_into()?;
    record_event(&mut *conn, intent.id, None, IntentStatus::Created, None).await?;
    Ok(intent)
}

pub async fn find(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<Intent>, StoreError> {
    let row: Option<Row> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM intents WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?;
    row.map(TryInto::try_into).transpose()
}

/// Moves the intent `from → to`, applying `changes`. Fails if the transition
/// is illegal or the intent is no longer in `from`.
pub async fn transition(
    conn: &mut PgConnection,
    id: Uuid,
    from: IntentStatus,
    to: IntentStatus,
    changes: Changes,
) -> Result<Intent, TransitionError> {
    state::check(from, to)?;
    let row: Option<Row> = sqlx::query_as(concat!(
        "UPDATE intents SET status = $3, \
         anchor_tx_id = COALESCE($4, anchor_tx_id), \
         interactive_url = COALESCE($5, interactive_url), \
         amount_asset = COALESCE($6::numeric, amount_asset), \
         intake_payment_tx_hash = COALESCE($7, intake_payment_tx_hash), \
         pool_deposit_tx_hash = COALESCE($8, pool_deposit_tx_hash), \
         pool_transfer_tx_hash = COALESCE($9, pool_transfer_tx_hash), \
         message = COALESCE($10, message), \
         completed_at = CASE WHEN $3 = 'completed' THEN now() ELSE completed_at END \
         WHERE id = $1 AND status = $2 RETURNING ",
        columns!()
    ))
    .bind(id)
    .bind(from.as_str())
    .bind(to.as_str())
    .bind(changes.anchor_tx_id)
    .bind(changes.interactive_url)
    .bind(changes.amount_asset.as_ref().map(DecimalAmount::as_str))
    .bind(changes.intake_payment_tx_hash.as_ref().map(TxHash::as_str))
    .bind(changes.pool_deposit_tx_hash.as_ref().map(TxHash::as_str))
    .bind(changes.pool_transfer_tx_hash.as_ref().map(TxHash::as_str))
    .bind(changes.message.as_deref())
    .fetch_optional(&mut *conn)
    .await?;
    let intent: Intent = row.ok_or(TransitionError::Stale(id))?.try_into()?;
    record_event(&mut *conn, id, Some(from), to, intent.message.as_deref()).await?;
    Ok(intent)
}

/// Status history, oldest first: `(from, to)`.
pub async fn events(
    db: impl PgExecutor<'_>,
    intent_id: Uuid,
) -> Result<Vec<(Option<IntentStatus>, IntentStatus)>, StoreError> {
    let rows: Vec<(Option<String>, String)> = sqlx::query_as(
        "SELECT from_status, to_status FROM intent_events WHERE intent_id = $1 \
         ORDER BY created_at, id",
    )
    .bind(intent_id)
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|(from, to)| {
            Ok((
                from.map(|f| f.parse().map_err(|e| corrupt("intent_events", e)))
                    .transpose()?,
                to.parse().map_err(|e| corrupt("intent_events", e))?,
            ))
        })
        .collect()
}

async fn record_event(
    conn: &mut PgConnection,
    intent_id: Uuid,
    from: Option<IntentStatus>,
    to: IntentStatus,
    message: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO intent_events (id, intent_id, from_status, to_status, message) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(intent_id)
    .bind(from.map(IntentStatus::as_str))
    .bind(to.as_str())
    .bind(message)
    .execute(conn)
    .await?;
    Ok(())
}
