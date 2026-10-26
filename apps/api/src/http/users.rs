//! Onboarding and user status (contract tags `Onboarding` and `ASP`).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::extract::{AdminAuth, ApiJson};
use crate::{
    asp::{
        AspStatus, KycScenario, KycStatus, ownership,
        service::{AspServiceError, Onboarded, OnboardingInput, UserView},
    },
    domain::{AccountId, PoolField, TxHash},
    error::ApiError,
    state::AppState,
};

/// Contract `OnboardingRequest`. Unknown fields are rejected so a client
/// can never send a spending key by mistake.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnboardingRequest {
    stellar_account: String,
    note_public_key: String,
    encryption_public_key: String,
    asp_leaf: String,
    /// SEP-53 signature by `stellar_account` of the onboarding message.
    signature: String,
    #[serde(default)]
    scenario: KycScenario,
}

/// Contract `AspActionRequest`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AspActionRequest {
    stellar_account: String,
}

/// Contract `ShieldUser`.
#[derive(Debug, Serialize)]
pub struct ShieldUserResponse {
    stellar_account: AccountId,
    kyc_status: KycStatus,
    asp_status: AspStatus,
    last_asp_tx_hash: Option<TxHash>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<UserView> for ShieldUserResponse {
    fn from(view: UserView) -> Self {
        Self {
            stellar_account: view.user.stellar_account,
            kyc_status: view.user.kyc_status,
            asp_status: view.user.asp_status,
            last_asp_tx_hash: view.last_asp_tx_hash,
            created_at: view.user.created_at,
            updated_at: view.user.updated_at,
        }
    }
}

pub async fn onboard(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<OnboardingRequest>,
) -> Result<(StatusCode, Json<ShieldUserResponse>), ApiError> {
    let input = OnboardingInput {
        stellar_account: parse_account(&body.stellar_account)?,
        note_public_key: parse_key("note_public_key", &body.note_public_key)?,
        encryption_public_key: parse_key("encryption_public_key", &body.encryption_public_key)?,
        asp_leaf: parse_key("asp_leaf", &body.asp_leaf)?,
        scenario: body.scenario,
    };
    // Both go into the pool's trees as BN254 field elements, in the byte
    // order the SDK prints them (leaf big-endian, note key little-endian).
    if !PoolField::is_canonical(&input.asp_leaf.field_be()) {
        return Err(not_a_field_element("asp_leaf"));
    }
    if !PoolField::is_canonical(&input.note_public_key.field_le()) {
        return Err(not_a_field_element("note_public_key"));
    }
    // Only the account's owner can attach pool keys to it.
    let message = ownership::onboarding_message(
        &input.stellar_account,
        &input.note_public_key,
        &input.encryption_public_key,
        &input.asp_leaf,
    );
    ownership::verify(&input.stellar_account, &message, &body.signature).map_err(|err| {
        ApiError::BadRequest {
            code: "invalid_signature",
            message: err.to_string(),
        }
    })?;
    Ok(match state.asp.onboard(input).await? {
        Onboarded::Created(view) => (StatusCode::CREATED, Json(view.into())),
        Onboarded::Existing(view) => (StatusCode::OK, Json(view.into())),
    })
}

pub async fn get_user(
    State(state): State<AppState>,
    Path(account): Path<String>,
) -> Result<Json<ShieldUserResponse>, ApiError> {
    let account = parse_account(&account)?;
    Ok(Json(state.asp.user(&account).await?.into()))
}

pub async fn block(
    _: AdminAuth,
    State(state): State<AppState>,
    ApiJson(body): ApiJson<AspActionRequest>,
) -> Result<Json<ShieldUserResponse>, ApiError> {
    let account = parse_account(&body.stellar_account)?;
    Ok(Json(state.asp.block(&account).await?.into()))
}

pub async fn unblock(
    _: AdminAuth,
    State(state): State<AppState>,
    ApiJson(body): ApiJson<AspActionRequest>,
) -> Result<Json<ShieldUserResponse>, ApiError> {
    let account = parse_account(&body.stellar_account)?;
    Ok(Json(state.asp.unblock(&account).await?.into()))
}

fn parse_account(raw: &str) -> Result<AccountId, ApiError> {
    AccountId::parse(raw).map_err(|e| ApiError::BadRequest {
        code: "invalid_account",
        message: e.to_string(),
    })
}

fn not_a_field_element(field: &str) -> ApiError {
    ApiError::BadRequest {
        code: "invalid_key",
        message: format!("{field} is not a valid pool field element"),
    }
}

fn parse_key(field: &str, raw: &str) -> Result<PoolField, ApiError> {
    PoolField::parse(raw).map_err(|_| ApiError::BadRequest {
        code: "invalid_key",
        message: format!("{field} must be a 0x-prefixed 32-byte hex value"),
    })
}

impl From<AspServiceError> for ApiError {
    fn from(err: AspServiceError) -> Self {
        match err {
            AspServiceError::AccountAlreadyOnboarded => Self::Conflict {
                code: "account_already_onboarded",
                message: "The account is already onboarded with different pool keys.".into(),
            },
            AspServiceError::PoolKeyTaken => Self::Conflict {
                code: "pool_key_already_registered",
                message: "These pool keys are registered to another account.".into(),
            },
            AspServiceError::UserNotFound => Self::Missing {
                code: "user_not_found",
                message: "The account has not been onboarded.".into(),
            },
            AspServiceError::Transition(err) => Self::Conflict {
                code: "asp_transition_not_allowed",
                message: err.to_string(),
            },
            AspServiceError::InFlight => Self::Conflict {
                code: "asp_update_in_flight",
                message: "An ASP update for this user is still in flight. Retry shortly.".into(),
            },
            AspServiceError::Store(err) => Self::Internal(Box::new(err)),
        }
    }
}
