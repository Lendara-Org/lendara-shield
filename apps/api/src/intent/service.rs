//! Creating and reading ramp intents.
//!
//! The intent is stored as `created` **before** calling the anchor, so every
//! request to the anchor has a matching row (and memo) even if the process
//! dies mid-call. The anchor's answer then moves it to `awaiting_anchor`, or
//! to `error` if the anchor could not start the deposit.
//!
//! Logs carry the intent id only, never the user's account with the amount
//! (AGENTS.md §5.2).

use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use super::state::IntentStatus;
use crate::{
    asp::AspStatus,
    domain::{AccountId, DecimalAmount, FiatCurrency},
    ports::{AnchorClient, AnchorError, StartDeposit},
    store::{
        StoreError,
        intents::{self, Changes, InsertIntentError, Intent, NewIntent, TransitionError},
        users,
    },
};

#[derive(Debug, Clone)]
pub struct CreateIntent {
    pub stellar_account: AccountId,
    pub fiat_currency: FiatCurrency,
    pub amount: DecimalAmount,
    /// Single-use id of the request, from the account's signed message.
    pub request_id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum IntentError {
    #[error("ramp mode is disabled")]
    RampDisabled,
    #[error("user not found")]
    UserNotFound,
    #[error("the user's key is not in the allow-list")]
    UserNotAllowed,
    #[error("intent not found")]
    NotFound,
    /// The signed request already created an intent.
    #[error("the request was already used")]
    Replayed,
    /// The intent was stored and moved to `error`.
    #[error("the anchor could not start the deposit")]
    AnchorUnavailable { intent_id: Uuid },
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<sqlx::Error> for IntentError {
    fn from(err: sqlx::Error) -> Self {
        Self::Store(err.into())
    }
}

#[derive(Debug, Clone)]
pub struct IntentService {
    db: PgPool,
    /// `None` disables ramp mode (`RAMP_ANCHOR_HOME_DOMAIN` unset).
    anchor: Option<Arc<dyn AnchorClient>>,
    /// Asset the pool holds and the anchor pays out (`USDC`).
    asset_code: String,
}

impl IntentService {
    #[must_use]
    pub fn new(db: PgPool, anchor: Option<Arc<dyn AnchorClient>>, asset_code: String) -> Self {
        Self {
            db,
            anchor,
            asset_code,
        }
    }

    pub async fn create(&self, input: CreateIntent) -> Result<Intent, IntentError> {
        let anchor = self.anchor.as_ref().ok_or(IntentError::RampDisabled)?;
        let user = users::find_by_account(&self.db, &input.stellar_account)
            .await?
            .ok_or(IntentError::UserNotFound)?;

        let mut tx = self.db.begin().await?;
        // Only keys the pool accepts can receive the private transfer. Read
        // with the user's row locked: a block confirmed by the ASP (which
        // takes the same lock and rejects unfunded intents) either happens
        // before and refuses this intent, or after and rejects it.
        if users::lock_by_id(&mut *tx, user.id).await?.asp_status != AspStatus::Allowed {
            return Err(IntentError::UserNotAllowed);
        }
        let intent = intents::insert_created(
            &mut tx,
            &NewIntent {
                user_id: user.id,
                request_id: input.request_id,
                fiat_currency: input.fiat_currency.clone(),
                amount_fiat: input.amount,
                asset_code: self.asset_code.clone(),
                anchor_home_domain: anchor.home_domain().to_owned(),
            },
        )
        .await
        .map_err(|err| match err {
            InsertIntentError::Replayed => IntentError::Replayed,
            InsertIntentError::Store(err) => IntentError::Store(err),
        })?;
        tx.commit().await?;
        tracing::info!(intent_id = %intent.id, "intent created");

        let request = StartDeposit {
            asset_code: self.asset_code.clone(),
            source_asset: input.fiat_currency.sep38_asset(),
            customer_memo: user.sep10_memo,
            deposit_memo: intent.memo,
        };
        let mut conn = self.db.acquire().await?;
        // While the anchor answers, the ASP may block the user: that rejects
        // this `created` intent (the only other writer of that status), so a
        // stale transition below means "not allowed", never a server error.
        // The user then never gets the interactive URL.
        let rejected_meanwhile = |err: TransitionError| match err {
            TransitionError::Stale(_) => {
                tracing::info!(intent_id = %intent.id, "intent rejected while the anchor answered");
                IntentError::UserNotAllowed
            }
            other => other.into(),
        };
        match anchor.start_deposit(&request).await {
            Ok(deposit) => {
                let intent = intents::transition(
                    &mut conn,
                    intent.id,
                    IntentStatus::Created,
                    IntentStatus::AwaitingAnchor,
                    Changes {
                        anchor_tx_id: Some(deposit.id),
                        interactive_url: Some(deposit.interactive_url),
                        ..Changes::default()
                    },
                )
                .await
                .map_err(rejected_meanwhile)?;
                tracing::info!(intent_id = %intent.id, "anchor deposit started");
                Ok(intent)
            }
            Err(err) => {
                let message = match &err {
                    AnchorError::Unavailable(_) => "The anchor is unavailable.",
                    AnchorError::Refused(_) => "The anchor refused the deposit.",
                };
                tracing::warn!(intent_id = %intent.id, error = %err, "anchor deposit not started");
                intents::transition(
                    &mut conn,
                    intent.id,
                    IntentStatus::Created,
                    IntentStatus::Error,
                    Changes {
                        message: Some(message.to_owned()),
                        ..Changes::default()
                    },
                )
                .await
                .map_err(rejected_meanwhile)?;
                Err(IntentError::AnchorUnavailable {
                    intent_id: intent.id,
                })
            }
        }
    }

    pub async fn get(&self, id: Uuid) -> Result<Intent, IntentError> {
        intents::find(&self.db, id)
            .await?
            .ok_or(IntentError::NotFound)
    }
}
