//! Audit (contract tag `Audit`): the auditor's decrypted view, behind
//! `AUDITOR_TOKEN`, and the public view of a transaction.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::extract::AuditorAuth;
use crate::{
    audit::{AuditError, AuditNoteView, AuditTxView},
    domain::{AccountId, PoolField, TxHash, amount::format_stroops},
    error::ApiError,
    ports::PublicTx,
    state::AppState,
};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;
const EXPLORER: &str = "https://stellar.expert/explorer/testnet/tx";

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    limit: Option<String>,
}

/// Contract `AuditTransactionList`.
#[derive(Debug, Serialize)]
pub struct AuditTransactionList {
    transactions: Vec<AuditTransaction>,
}

/// Contract `AuditTransaction`.
#[derive(Debug, Serialize)]
struct AuditTransaction {
    ledger: u32,
    tx_hash: Option<TxHash>,
    kind: &'static str,
    ext_amount: String,
    inputs: Vec<AuditNote>,
    outputs: Vec<AuditNote>,
}

/// Contract `AuditNote`.
#[derive(Debug, Serialize)]
struct AuditNote {
    owner_note_public_key: PoolField,
    owner_stellar_account: Option<AccountId>,
    amount: String,
}

/// Contract `PublicTransaction`.
#[derive(Debug, Serialize)]
pub struct PublicTransaction {
    tx_hash: TxHash,
    ledger: u32,
    created_at: DateTime<Utc>,
    source_account: AccountId,
    is_pool_transaction: bool,
    ext_amount: Option<String>,
    visible_accounts: Vec<AccountId>,
    explorer_url: String,
}

pub async fn transactions(
    _auth: AuditorAuth,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<AuditTransactionList>, ApiError> {
    let limit = match query.limit.as_deref() {
        None => DEFAULT_LIMIT,
        Some(raw) => raw
            .parse()
            .ok()
            .filter(|n| (1..=MAX_LIMIT).contains(n))
            .ok_or_else(|| ApiError::BadRequest {
                code: "invalid_limit",
                message: format!("limit must be an integer from 1 to {MAX_LIMIT}"),
            })?,
    };
    let views = state.audit.transactions(limit).await.map_err(audit_error)?;
    Ok(Json(AuditTransactionList {
        transactions: views.into_iter().map(AuditTransaction::from).collect(),
    }))
}

pub async fn public(
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> Result<Json<PublicTransaction>, ApiError> {
    let hash = TxHash::parse(&raw).map_err(|e| ApiError::BadRequest {
        code: "invalid_tx_hash",
        message: e.to_string(),
    })?;
    let tx = state
        .audit
        .public(&hash)
        .await
        .map_err(audit_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(tx.into()))
}

fn audit_error(err: AuditError) -> ApiError {
    match err {
        AuditError::NotConfigured | AuditError::Unavailable(_) => {
            ApiError::Unavailable(err.to_string())
        }
        AuditError::Store(err) => ApiError::Internal(err.into()),
    }
}

impl From<AuditTxView> for AuditTransaction {
    fn from(view: AuditTxView) -> Self {
        let notes = |notes: Vec<AuditNoteView>| notes.into_iter().map(AuditNote::from).collect();
        Self {
            ledger: view.ledger,
            tx_hash: view.tx_hash,
            kind: view.kind.as_str(),
            ext_amount: view.ext_amount,
            inputs: notes(view.inputs),
            outputs: notes(view.outputs),
        }
    }
}

impl From<AuditNoteView> for AuditNote {
    fn from(view: AuditNoteView) -> Self {
        Self {
            owner_note_public_key: view.owner_note_public_key,
            owner_stellar_account: view.owner_stellar_account,
            amount: view.amount,
        }
    }
}

impl From<PublicTx> for PublicTransaction {
    fn from(tx: PublicTx) -> Self {
        Self {
            explorer_url: format!("{EXPLORER}/{}", tx.hash),
            tx_hash: tx.hash,
            ledger: tx.ledger,
            created_at: tx.created_at,
            source_account: tx.source_account,
            is_pool_transaction: tx.is_pool_transaction,
            ext_amount: tx.pool_ext_stroops.map(format_stroops),
            visible_accounts: tx.visible_accounts,
        }
    }
}
