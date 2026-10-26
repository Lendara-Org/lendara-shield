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
    ports::PoolTx,
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
    pub pool_deposit: Option<PoolTx>,
    pub pool_transfer: Option<PoolTx>,
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
    pub pool_deposit: Option<PoolTx>,
    pub pool_transfer: Option<PoolTx>,
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
         interactive_url, intake_payment_tx_hash, pool_deposit_tx_hash, pool_deposit_envelope, \
         pool_deposit_sequence, pool_transfer_tx_hash, pool_transfer_envelope, \
         pool_transfer_sequence, message, created_at, updated_at, completed_at"
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
    pool_deposit_envelope: Option<String>,
    pool_deposit_sequence: Option<i64>,
    pool_transfer_tx_hash: Option<String>,
    pool_transfer_envelope: Option<String>,
    pool_transfer_sequence: Option<i64>,
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

fn pool_tx(
    hash: Option<String>,
    envelope: Option<String>,
    sequence: Option<i64>,
) -> Result<Option<PoolTx>, StoreError> {
    match (hash, envelope, sequence) {
        (Some(hash), Some(envelope_xdr), Some(sequence)) => Ok(Some(PoolTx {
            hash: TxHash::parse(&hash).map_err(|e| corrupt(TABLE, e))?,
            envelope_xdr,
            sequence,
        })),
        (None, None, None) => Ok(None),
        _ => Err(corrupt(TABLE, "incomplete pool transaction")),
    }
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
            pool_deposit: pool_tx(
                row.pool_deposit_tx_hash,
                row.pool_deposit_envelope,
                row.pool_deposit_sequence,
            )?,
            pool_transfer: pool_tx(
                row.pool_transfer_tx_hash,
                row.pool_transfer_envelope,
                row.pool_transfer_sequence,
            )?,
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
         pool_deposit_envelope = COALESCE($11, pool_deposit_envelope), \
         pool_deposit_sequence = COALESCE($12, pool_deposit_sequence), \
         pool_transfer_tx_hash = COALESCE($9, pool_transfer_tx_hash), \
         pool_transfer_envelope = COALESCE($13, pool_transfer_envelope), \
         pool_transfer_sequence = COALESCE($14, pool_transfer_sequence), \
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
    .bind(changes.pool_deposit.as_ref().map(|tx| tx.hash.as_str()))
    .bind(changes.pool_transfer.as_ref().map(|tx| tx.hash.as_str()))
    .bind(changes.message.as_deref())
    .bind(
        changes
            .pool_deposit
            .as_ref()
            .map(|tx| tx.envelope_xdr.as_str()),
    )
    .bind(changes.pool_deposit.as_ref().map(|tx| tx.sequence))
    .bind(
        changes
            .pool_transfer
            .as_ref()
            .map(|tx| tx.envelope_xdr.as_str()),
    )
    .bind(changes.pool_transfer.as_ref().map(|tx| tx.sequence))
    .fetch_optional(&mut *conn)
    .await?;
    let intent: Intent = row.ok_or(TransitionError::Stale(id))?.try_into()?;
    record_event(&mut *conn, id, Some(from), to, intent.message.as_deref()).await?;
    Ok(intent)
}

/// Locks the intent with this SEP-24 memo until the transaction ends.
pub async fn lock_by_memo(
    conn: &mut PgConnection,
    memo: i64,
) -> Result<Option<Intent>, StoreError> {
    let row: Option<Row> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM intents WHERE memo = $1 FOR UPDATE"
    ))
    .bind(memo)
    .fetch_optional(&mut *conn)
    .await?;
    row.map(TryInto::try_into).transpose()
}

/// Intents waiting for the anchor whose next check is due, with the user's
/// customer memo (to authenticate as them with the anchor).
pub async fn due_awaiting_anchor(
    db: impl PgExecutor<'_>,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<(Intent, i64)>, StoreError> {
    #[derive(FromRow)]
    struct DueRow {
        #[sqlx(flatten)]
        intent: Row,
        customer_memo: i64,
    }
    let rows: Vec<DueRow> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        ", (SELECT sep10_memo FROM shield_users u WHERE u.id = intents.user_id) AS customer_memo \
         FROM intents WHERE status = 'awaiting_anchor' \
         AND (next_attempt_at IS NULL OR next_attempt_at <= $1) \
         ORDER BY next_attempt_at NULLS FIRST, created_at LIMIT $2"
    ))
    .bind(now)
    .bind(limit)
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|row| Ok((row.intent.try_into()?, row.customer_memo)))
        .collect()
}

/// Makes the intent due now (e.g. a payment with its memo just arrived).
pub async fn check_now(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), StoreError> {
    sqlx::query("UPDATE intents SET next_attempt_at = NULL WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Sets when the intent is next looked at.
pub async fn schedule(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: DateTime<Utc>,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE intents SET next_attempt_at = $2 WHERE id = $1")
        .bind(id)
        .bind(at)
        .execute(db)
        .await?;
    Ok(())
}

/// Which pool transaction of an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolStep {
    Deposit,
    Transfer,
}

/// Replaces the stored pool transaction of `step` (a new transaction after
/// the previous one can never land), only while the intent is in `status`.
pub async fn replace_pool_tx(
    conn: &mut PgConnection,
    id: Uuid,
    status: IntentStatus,
    step: PoolStep,
    tx: &PoolTx,
) -> Result<(), TransitionError> {
    let sql = match step {
        PoolStep::Deposit => {
            "UPDATE intents SET pool_deposit_tx_hash = $3, pool_deposit_envelope = $4, \
             pool_deposit_sequence = $5 WHERE id = $1 AND status = $2"
        }
        PoolStep::Transfer => {
            "UPDATE intents SET pool_transfer_tx_hash = $3, pool_transfer_envelope = $4, \
             pool_transfer_sequence = $5 WHERE id = $1 AND status = $2"
        }
    };
    let updated = sqlx::query(sql)
        .bind(id)
        .bind(status.as_str())
        .bind(tx.hash.as_str())
        .bind(&tx.envelope_xdr)
        .bind(tx.sequence)
        .execute(conn)
        .await?
        .rows_affected();
    if updated == 1 {
        Ok(())
    } else {
        Err(TransitionError::Stale(id))
    }
}

/// An intent the shielding worker should advance, with the recipient's keys.
#[derive(Debug, Clone)]
pub struct WorkItem {
    pub intent: Intent,
    pub attempts: i32,
    /// See [`set_unseen_since`].
    pub unseen_since: Option<DateTime<Utc>>,
    pub note_public_key: crate::domain::PoolField,
    pub encryption_public_key: crate::domain::PoolField,
}

/// Intents in `funds_received`, `shielding` or `transferring` that are due.
pub async fn due_for_shielding(
    db: impl PgExecutor<'_>,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<WorkItem>, StoreError> {
    #[derive(FromRow)]
    struct WorkRow {
        #[sqlx(flatten)]
        intent: Row,
        attempts: i32,
        pool_unseen_since: Option<DateTime<Utc>>,
        note_public_key: String,
        encryption_public_key: String,
    }
    let rows: Vec<WorkRow> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        ", attempts, pool_unseen_since, (SELECT note_public_key FROM shield_users u WHERE u.id = intents.user_id) \
         AS note_public_key, (SELECT encryption_public_key FROM shield_users u \
         WHERE u.id = intents.user_id) AS encryption_public_key \
         FROM intents WHERE status IN ('funds_received', 'shielding', 'transferring') \
         AND (next_attempt_at IS NULL OR next_attempt_at <= $1) \
         ORDER BY next_attempt_at NULLS FIRST, created_at LIMIT $2"
    ))
    .bind(now)
    .bind(limit)
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|row| {
            let key =
                |raw: &str| crate::domain::PoolField::parse(raw).map_err(|e| corrupt(TABLE, e));
            Ok(WorkItem {
                note_public_key: key(&row.note_public_key)?,
                encryption_public_key: key(&row.encryption_public_key)?,
                attempts: row.attempts,
                unseen_since: row.pool_unseen_since,
                intent: row.intent.try_into()?,
            })
        })
        .collect()
}

/// Marks (or clears, with `None`) when the current pool transaction was
/// first unknown with its sequence already passed.
pub async fn set_unseen_since(
    db: impl PgExecutor<'_>,
    id: Uuid,
    since: Option<DateTime<Utc>>,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE intents SET pool_unseen_since = $2 WHERE id = $1")
        .bind(id)
        .bind(since)
        .execute(db)
        .await?;
    Ok(())
}

/// Records a failed attempt and when to try again.
pub async fn record_attempt(
    db: impl PgExecutor<'_>,
    id: Uuid,
    next_at: DateTime<Utc>,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE intents SET attempts = attempts + 1, next_attempt_at = $2 WHERE id = $1")
        .bind(id)
        .bind(next_at)
        .execute(db)
        .await?;
    Ok(())
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
