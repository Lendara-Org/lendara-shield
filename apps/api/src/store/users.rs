//! `shield_users`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, prelude::FromRow};
use uuid::Uuid;

use super::{StoreError, corrupt, unique_violation};
use crate::{
    asp::{AspStatus, KycScenario, KycStatus},
    domain::{AccountId, PoolField},
    ports::AspKeys,
};

const TABLE: &str = "shield_users";

/// A Shield user as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldUser {
    pub id: Uuid,
    pub stellar_account: AccountId,
    pub sep10_memo: i64,
    pub note_public_key: PoolField,
    pub encryption_public_key: PoolField,
    pub asp_leaf: PoolField,
    pub kyc_scenario: KycScenario,
    pub kyc_status: KycStatus,
    pub asp_status: AspStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ShieldUser {
    #[must_use]
    pub fn asp_keys(&self) -> AspKeys {
        AspKeys {
            asp_leaf: self.asp_leaf.clone(),
            note_public_key: self.note_public_key.clone(),
        }
    }
}

/// Fields of a new user.
#[derive(Debug, Clone)]
pub struct NewUser {
    pub stellar_account: AccountId,
    pub note_public_key: PoolField,
    pub encryption_public_key: PoolField,
    pub asp_leaf: PoolField,
    pub kyc_scenario: KycScenario,
    pub kyc_status: KycStatus,
    pub asp_status: AspStatus,
}

#[derive(Debug, thiserror::Error)]
pub enum InsertUserError {
    #[error("the account is already onboarded")]
    AccountTaken,
    #[error("the pool keys belong to another account")]
    PoolKeyTaken,
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(FromRow)]
struct Row {
    id: Uuid,
    stellar_account: String,
    sep10_memo: i64,
    note_public_key: String,
    encryption_public_key: String,
    asp_leaf: String,
    kyc_scenario: String,
    kyc_status: String,
    asp_status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<Row> for ShieldUser {
    type Error = StoreError;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            stellar_account: AccountId::parse(&row.stellar_account)
                .map_err(|e| corrupt(TABLE, e))?,
            sep10_memo: row.sep10_memo,
            note_public_key: PoolField::parse(&row.note_public_key)
                .map_err(|e| corrupt(TABLE, e))?,
            encryption_public_key: PoolField::parse(&row.encryption_public_key)
                .map_err(|e| corrupt(TABLE, e))?,
            asp_leaf: PoolField::parse(&row.asp_leaf).map_err(|e| corrupt(TABLE, e))?,
            kyc_scenario: row.kyc_scenario.parse().map_err(|e| corrupt(TABLE, e))?,
            kyc_status: row.kyc_status.parse().map_err(|e| corrupt(TABLE, e))?,
            asp_status: row.asp_status.parse().map_err(|e| corrupt(TABLE, e))?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Column list shared by every query, as a macro so queries stay `&'static str`
/// (sqlx 0.9 refuses dynamically built SQL).
macro_rules! columns {
    () => {
        "id, stellar_account, sep10_memo, note_public_key, encryption_public_key, \
         asp_leaf, kyc_scenario, kyc_status, asp_status, created_at, updated_at"
    };
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    user: &NewUser,
) -> Result<ShieldUser, InsertUserError> {
    let row: Row = sqlx::query_as(concat!(
        "INSERT INTO shield_users (id, stellar_account, note_public_key, encryption_public_key, \
         asp_leaf, kyc_scenario, kyc_status, asp_status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING ",
        columns!()
    ))
    .bind(Uuid::now_v7())
    .bind(user.stellar_account.as_str())
    .bind(user.note_public_key.as_str())
    .bind(user.encryption_public_key.as_str())
    .bind(user.asp_leaf.as_str())
    .bind(user.kyc_scenario.as_str())
    .bind(user.kyc_status.as_str())
    .bind(user.asp_status.as_str())
    .fetch_one(db)
    .await
    .map_err(|err| match unique_violation(&err) {
        Some("shield_users_stellar_account_key") => InsertUserError::AccountTaken,
        Some("shield_users_note_public_key_key" | "shield_users_asp_leaf_key") => {
            InsertUserError::PoolKeyTaken
        }
        _ => InsertUserError::Store(err.into()),
    })?;
    Ok(row.try_into()?)
}

pub async fn find_by_account(
    db: impl PgExecutor<'_>,
    account: &AccountId,
) -> Result<Option<ShieldUser>, StoreError> {
    let row: Option<Row> = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM shield_users WHERE stellar_account = $1"
    ))
    .bind(account.as_str())
    .fetch_optional(db)
    .await?;
    row.map(TryInto::try_into).transpose()
}

/// Accounts of the users who registered these note public keys.
pub async fn accounts_by_note_keys(
    db: impl PgExecutor<'_>,
    keys: &[PoolField],
) -> Result<HashMap<PoolField, AccountId>, StoreError> {
    let keys: Vec<&str> = keys.iter().map(PoolField::as_str).collect();
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT note_public_key, stellar_account FROM shield_users WHERE note_public_key = ANY($1)",
    )
    .bind(&keys)
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|(key, account)| {
            Ok((
                PoolField::parse(&key).map_err(|e| corrupt("shield_users", e))?,
                AccountId::parse(&account).map_err(|e| corrupt("shield_users", e))?,
            ))
        })
        .collect()
}

/// Reads the user and locks the row until the transaction ends, so ASP
/// decisions for one user never interleave.
pub async fn lock_by_id(db: impl PgExecutor<'_>, id: Uuid) -> Result<ShieldUser, StoreError> {
    let row: Row = sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM shield_users WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_one(db)
    .await?;
    row.try_into()
}

pub async fn set_asp_status(
    db: impl PgExecutor<'_>,
    id: Uuid,
    status: AspStatus,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE shield_users SET asp_status = $2 WHERE id = $1")
        .bind(id)
        .bind(status.as_str())
        .execute(db)
        .await?;
    Ok(())
}
