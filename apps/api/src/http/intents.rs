//! Ramp intents (contract tag `Intents`).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::extract::ApiJson;
use crate::{
    domain::{AccountId, DecimalAmount, FiatCurrency, TxHash},
    error::ApiError,
    intent::{
        IntentStatus,
        service::{CreateIntent, IntentError},
    },
    state::AppState,
    store::intents::Intent,
};

/// Contract `CreateIntentRequest`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateIntentRequest {
    mode: Mode,
    stellar_account: String,
    fiat_currency: String,
    amount: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Ramp,
}

/// Contract `Intent`.
#[derive(Debug, Serialize)]
pub struct IntentResponse {
    id: Uuid,
    mode: Mode,
    status: IntentStatus,
    fiat_currency: FiatCurrency,
    amount_fiat: DecimalAmount,
    asset_code: String,
    amount_asset: Option<DecimalAmount>,
    interactive_url: Option<String>,
    transactions: Transactions,
    message: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct Transactions {
    intake_payment: Option<TxHash>,
    pool_deposit: Option<TxHash>,
    pool_transfer: Option<TxHash>,
}

impl From<Intent> for IntentResponse {
    fn from(intent: Intent) -> Self {
        Self {
            id: intent.id,
            mode: Mode::Ramp,
            // The URL is only useful while the user still has to act.
            interactive_url: intent
                .interactive_url
                .filter(|_| intent.status == IntentStatus::AwaitingAnchor),
            status: intent.status,
            fiat_currency: intent.fiat_currency,
            amount_fiat: intent.amount_fiat,
            asset_code: intent.asset_code,
            amount_asset: intent.amount_asset,
            transactions: Transactions {
                intake_payment: intent.intake_payment_tx_hash,
                pool_deposit: intent.pool_deposit.map(|tx| tx.hash),
                pool_transfer: intent.pool_transfer.map(|tx| tx.hash),
            },
            message: intent.message,
            created_at: intent.created_at,
            updated_at: intent.updated_at,
            completed_at: intent.completed_at,
        }
    }
}

pub async fn create(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<CreateIntentRequest>,
) -> Result<(StatusCode, Json<IntentResponse>), ApiError> {
    let Mode::Ramp = body.mode;
    let input = CreateIntent {
        stellar_account: AccountId::parse(&body.stellar_account).map_err(|e| {
            ApiError::BadRequest {
                code: "invalid_account",
                message: e.to_string(),
            }
        })?,
        fiat_currency: FiatCurrency::parse(&body.fiat_currency).map_err(|e| {
            ApiError::BadRequest {
                code: "invalid_currency",
                message: e.to_string(),
            }
        })?,
        amount: DecimalAmount::parse(&body.amount).map_err(|e| ApiError::BadRequest {
            code: "invalid_amount",
            message: e.to_string(),
        })?,
    };
    let intent = state.intents.create(input).await?;
    Ok((StatusCode::CREATED, Json(intent.into())))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<IntentResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest {
        code: "invalid_request",
        message: "intent id must be a UUID".into(),
    })?;
    Ok(Json(state.intents.get(id).await?.into()))
}

impl From<IntentError> for ApiError {
    fn from(err: IntentError) -> Self {
        match err {
            IntentError::RampDisabled => Self::Conflict {
                code: "ramp_mode_disabled",
                message: "Ramp mode is not configured on this server.".into(),
            },
            IntentError::UserNotAllowed => Self::Conflict {
                code: "user_not_allowed",
                message: "The account's pool key is not in the allow-list.".into(),
            },
            IntentError::UserNotFound => Self::Missing {
                code: "user_not_found",
                message: "The account has not been onboarded.".into(),
            },
            IntentError::NotFound => Self::Missing {
                code: "intent_not_found",
                message: "No intent with this id.".into(),
            },
            IntentError::AnchorUnavailable { .. } => Self::BadGateway {
                code: "anchor_unavailable",
                message: "The anchor could not start the deposit. Try again later.".into(),
            },
            IntentError::Transition(err) => Self::Internal(Box::new(err)),
            IntentError::Store(err) => Self::Internal(Box::new(err)),
        }
    }
}
