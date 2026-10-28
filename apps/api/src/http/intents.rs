//! Ramp intents (contract tag `Intents`).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::time::Duration;

use super::extract::ApiJson;
use crate::{
    asp::ownership::{self, OwnershipError},
    domain::{AccountId, DecimalAmount, FiatCurrency, TxHash},
    error::ApiError,
    intent::{
        IntentStatus,
        service::{CreateIntent, IntentError},
    },
    state::AppState,
    store::intents::Intent,
};

/// How old a signed intent request may be (`issued_at`).
pub const MAX_REQUEST_AGE: Duration = Duration::from_secs(300);
/// How far ahead of Shield's clock `issued_at` may be.
pub const MAX_CLOCK_SKEW: Duration = Duration::from_secs(60);

/// Contract `CreateIntentRequest`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateIntentRequest {
    mode: Mode,
    stellar_account: String,
    fiat_currency: String,
    amount: String,
    /// Single-use id chosen by the client (any UUID).
    request_id: String,
    /// Unix seconds when the client signed the request.
    issued_at: i64,
    /// SEP-53 signature by `stellar_account` of the intent message.
    signature: String,
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
    let request_id = Uuid::parse_str(&body.request_id).map_err(|_| ApiError::BadRequest {
        code: "invalid_request",
        message: "request_id must be a UUID".into(),
    })?;
    let input = CreateIntent {
        request_id,
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
    check_fresh(body.issued_at, state.clock.now())?;
    // Only the account's owner can start a deposit for it: the signature
    // covers the amount, and the request id makes it single-use.
    let message = ownership::intent_message(
        &input.stellar_account,
        &body.fiat_currency,
        &body.amount,
        &request_id,
        body.issued_at,
    );
    ownership::verify(&input.stellar_account, &message, &body.signature).map_err(|err| {
        ApiError::BadRequest {
            code: "invalid_signature",
            // The shared text speaks of onboarding keys; say what an intent signs.
            message: match err {
                OwnershipError::Invalid => {
                    "the signature is not the account's signature over this request \
                                            (account, currency, amount, request_id and issued_at)"
                        .into()
                }
                OwnershipError::Malformed => err.to_string(),
            },
        }
    })?;
    let intent = state.intents.create(input).await?;
    Ok((StatusCode::CREATED, Json(intent.into())))
}

/// `issued_at` must be recent, so a captured request cannot be used later.
fn check_fresh(issued_at: i64, now: DateTime<Utc>) -> Result<(), ApiError> {
    let age = now.timestamp().saturating_sub(issued_at);
    let max_age = i64::try_from(MAX_REQUEST_AGE.as_secs()).unwrap_or(i64::MAX);
    let max_skew = i64::try_from(MAX_CLOCK_SKEW.as_secs()).unwrap_or(i64::MAX);
    if age > max_age || age < -max_skew {
        return Err(ApiError::BadRequest {
            code: "request_expired",
            message: "issued_at is too far from the current time; sign the request again.".into(),
        });
    }
    Ok(())
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<IntentResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest {
        code: "invalid_request",
        message: "intent id must be a UUID".into(),
    })?;
    let mut intent: IntentResponse = state.intents.get(id).await?.into();
    // The interactive URL carries the anchor's one-time session token. It
    // goes only to the signed request that created the intent, never to
    // whoever knows the intent id.
    intent.interactive_url = None;
    Ok(Json(intent))
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
            IntentError::Replayed => Self::Conflict {
                code: "request_replayed",
                message: "This signed request was already used; sign a new one.".into(),
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
