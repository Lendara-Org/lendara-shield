//! Onboarding and ASP administration: orchestrates the database and the
//! on-chain [`AspContract`].
//!
//! Money-integrity rules (CLAUDE.md §0.6) applied to ASP writes:
//! - the transaction hash is stored **before** sending;
//! - a retry first asks the network about the stored hash and never resends
//!   while it may still land (until its `valid_until` time bound passes);
//! - at most one update per user is in flight (unique index in the DB).
//!
//! Chain failures do not fail the request: the user stays `pending` and the
//! next onboarding call (or `GET /v1/users`) picks up where it stopped.

use std::{sync::Arc, time::Duration};

use sqlx::PgPool;

use super::rules::{self, AspAction, AspStatus, KycScenario, TransitionError};
use crate::{
    domain::{AccountId, PoolField, TxHash},
    ports::{AspContract, ChainError, Clock, TxStatus},
    store::{
        StoreError,
        asp_events::{self, RecordError, SubmittedEvent},
        users::{self, InsertUserError, NewUser, ShieldUser},
    },
};

/// How long a request waits for an ASP transaction to land.
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// New transactions one request may build after failures, so a network that
/// keeps rejecting cannot loop forever.
const MAX_ATTEMPTS_PER_REQUEST: usize = 3;

/// What the web sends at onboarding (contract `OnboardingRequest`).
#[derive(Debug, Clone)]
pub struct OnboardingInput {
    pub stellar_account: AccountId,
    pub note_public_key: PoolField,
    pub encryption_public_key: PoolField,
    pub asp_leaf: PoolField,
    pub scenario: KycScenario,
}

/// A user and the hash of their latest ASP update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserView {
    pub user: ShieldUser,
    pub last_asp_tx_hash: Option<TxHash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Onboarded {
    Created(UserView),
    /// Same account and keys as an earlier call: idempotent replay.
    Existing(UserView),
}

#[derive(Debug, thiserror::Error)]
pub enum AspServiceError {
    #[error("the account is already onboarded with different pool keys")]
    AccountAlreadyOnboarded,
    #[error("the pool keys are registered to another account")]
    PoolKeyTaken,
    #[error("user not found")]
    UserNotFound,
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error("an ASP update is already in flight for this user")]
    InFlight,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<sqlx::Error> for AspServiceError {
    fn from(err: sqlx::Error) -> Self {
        Self::Store(err.into())
    }
}

/// Outcome of driving one on-chain update.
enum Drive {
    Confirmed,
    /// Still pending, or the chain is unavailable: try again later.
    Unsettled,
    /// Will never land; a new transaction is needed.
    Failed,
}

#[derive(Debug, Clone)]
pub struct AspService {
    db: PgPool,
    chain: Arc<dyn AspContract>,
    clock: Arc<dyn Clock>,
    confirm_timeout: Duration,
    poll_interval: Duration,
}

impl AspService {
    #[must_use]
    pub fn new(db: PgPool, chain: Arc<dyn AspContract>, clock: Arc<dyn Clock>) -> Self {
        Self {
            db,
            chain,
            clock,
            confirm_timeout: CONFIRM_TIMEOUT,
            poll_interval: POLL_INTERVAL,
        }
    }

    /// Overrides how long requests wait for confirmations (tests).
    #[must_use]
    pub fn with_timing(mut self, confirm_timeout: Duration, poll_interval: Duration) -> Self {
        self.confirm_timeout = confirm_timeout;
        self.poll_interval = poll_interval;
        self
    }

    /// Registers the user, runs the simulated KYC and drives the on-chain
    /// ASP updates it requires. Idempotent per account and keys.
    pub async fn onboard(&self, input: OnboardingInput) -> Result<Onboarded, AspServiceError> {
        let (user, created) = match users::find_by_account(&self.db, &input.stellar_account).await?
        {
            Some(existing) => (Self::same_keys(existing, &input)?, false),
            None => match self.insert(&input).await {
                Ok(user) => (user, true),
                // Lost a race against a concurrent call for the same account.
                Err(AspServiceError::AccountAlreadyOnboarded) => {
                    let existing = users::find_by_account(&self.db, &input.stellar_account)
                        .await?
                        .ok_or(AspServiceError::UserNotFound)?;
                    (Self::same_keys(existing, &input)?, false)
                }
                Err(err) => return Err(err),
            },
        };

        self.settle_onboarding(&user).await?;
        let view = self.view(&input.stellar_account).await?;
        Ok(if created {
            Onboarded::Created(view)
        } else {
            Onboarded::Existing(view)
        })
    }

    /// Current status. If an update is in flight, asks the network about it
    /// first (read-only: never sends a transaction).
    pub async fn user(&self, account: &AccountId) -> Result<UserView, AspServiceError> {
        let user = self.find(account).await?;
        if let Some(event) = asp_events::find_submitted(&self.db, user.id).await? {
            self.check_once(&user, &event).await?;
        }
        self.view(account).await
    }

    /// Admin: freezes the user's key (block-list insert).
    pub async fn block(&self, account: &AccountId) -> Result<UserView, AspServiceError> {
        self.admin_action(account, AspAction::Block).await
    }

    /// Admin: lifts a freeze (block-list delete).
    pub async fn unblock(&self, account: &AccountId) -> Result<UserView, AspServiceError> {
        self.admin_action(account, AspAction::Unblock).await
    }

    async fn admin_action(
        &self,
        account: &AccountId,
        action: AspAction,
    ) -> Result<UserView, AspServiceError> {
        let user = self.find(account).await?;
        // Admin actions need a settled user: `pending` belongs to onboarding.
        if user.asp_status == AspStatus::Pending {
            return Err(TransitionError {
                action,
                status: user.asp_status,
            }
            .into());
        }
        rules::check_transition(user.asp_status, action)?;
        self.run(&user, action).await?;
        self.view(account).await
    }

    async fn insert(&self, input: &OnboardingInput) -> Result<ShieldUser, AspServiceError> {
        let outcome = rules::run_kyc(input.scenario);
        let new_user = NewUser {
            stellar_account: input.stellar_account.clone(),
            note_public_key: input.note_public_key.clone(),
            encryption_public_key: input.encryption_public_key.clone(),
            asp_leaf: input.asp_leaf.clone(),
            kyc_scenario: input.scenario,
            kyc_status: outcome.kyc_status,
            asp_status: outcome.asp_status,
        };
        users::insert(&self.db, &new_user)
            .await
            .map_err(|err| match err {
                InsertUserError::AccountTaken => AspServiceError::AccountAlreadyOnboarded,
                InsertUserError::PoolKeyTaken => AspServiceError::PoolKeyTaken,
                InsertUserError::Store(err) => err.into(),
            })
    }

    fn same_keys(
        existing: ShieldUser,
        input: &OnboardingInput,
    ) -> Result<ShieldUser, AspServiceError> {
        let same = existing.note_public_key == input.note_public_key
            && existing.encryption_public_key == input.encryption_public_key
            && existing.asp_leaf == input.asp_leaf;
        if same {
            Ok(existing)
        } else {
            Err(AspServiceError::AccountAlreadyOnboarded)
        }
    }

    /// Drives the onboarding actions still owed, in order, stopping at the
    /// first one that does not settle within this request.
    async fn settle_onboarding(&self, user: &ShieldUser) -> Result<(), AspServiceError> {
        if user.asp_status != AspStatus::Pending {
            return Ok(());
        }
        let mut attempts = 0;
        loop {
            if let Some(event) = asp_events::find_submitted(&self.db, user.id).await? {
                match self.wait_for(user, &event).await? {
                    Drive::Confirmed | Drive::Failed => continue,
                    Drive::Unsettled => return Ok(()),
                }
            }

            let confirmed = asp_events::confirmed_actions(&self.db, user.id).await?;
            let remaining = rules::remaining_onboarding_actions(user.kyc_scenario, &confirmed);
            let Some(&next) = remaining.first() else {
                return Ok(());
            };
            if attempts == MAX_ATTEMPTS_PER_REQUEST {
                tracing::warn!(user_id = %user.id, action = %next, "ASP update still failing; leaving user pending");
                return Ok(());
            }
            attempts += 1;
            match self.run(user, next).await {
                Ok(Drive::Confirmed | Drive::Failed) => {}
                Ok(Drive::Unsettled) | Err(AspServiceError::InFlight) => return Ok(()),
                Err(err) => return Err(err),
            }
        }
    }

    /// Prepares, records, sends and waits for one ASP update.
    async fn run(&self, user: &ShieldUser, action: AspAction) -> Result<Drive, AspServiceError> {
        let prepared = match self.chain.prepare(action, &user.asp_keys()).await {
            Ok(prepared) => prepared,
            Err(err) => {
                tracing::warn!(user_id = %user.id, %action, error = %err, "ASP update not prepared");
                return Ok(Drive::Unsettled);
            }
        };

        // Persist the hash before sending (§0.6).
        let event = asp_events::record_submitted(
            &self.db,
            user.id,
            action,
            &prepared.hash,
            prepared.valid_until,
        )
        .await
        .map_err(|err| match err {
            RecordError::InFlight => AspServiceError::InFlight,
            RecordError::Store(err) => err.into(),
        })?;
        tracing::info!(user_id = %user.id, %action, tx_hash = %prepared.hash, "ASP update submitted");

        match self.chain.submit(&prepared).await {
            Ok(()) => {}
            Err(ChainError::Rejected(reason)) => {
                asp_events::mark_failed(&self.db, event.id, &reason).await?;
                tracing::warn!(user_id = %user.id, %action, %reason, "ASP update rejected");
                return Ok(Drive::Failed);
            }
            // Ambiguous: the transaction may still land. The stored hash
            // decides on the next check; never resend blindly.
            Err(ChainError::Unavailable(reason)) => {
                tracing::warn!(user_id = %user.id, %action, %reason, "ASP submit outcome unknown");
            }
        }

        self.wait_for(user, &event).await
    }

    /// Polls the network until the event settles or the request budget runs out.
    async fn wait_for(
        &self,
        user: &ShieldUser,
        event: &SubmittedEvent,
    ) -> Result<Drive, AspServiceError> {
        let deadline = tokio::time::Instant::now() + self.confirm_timeout;
        loop {
            match self.check_once(user, event).await? {
                Drive::Unsettled if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(self.poll_interval).await;
                }
                outcome => return Ok(outcome),
            }
        }
    }

    /// One read-only look at the network for an in-flight event.
    async fn check_once(
        &self,
        user: &ShieldUser,
        event: &SubmittedEvent,
    ) -> Result<Drive, AspServiceError> {
        let status = match self.chain.status(&event.tx_hash).await {
            Ok(status) => status,
            Err(err) => {
                tracing::warn!(user_id = %user.id, tx_hash = %event.tx_hash, error = %err, "ASP status unknown");
                return Ok(Drive::Unsettled);
            }
        };
        match status {
            TxStatus::Success => {
                self.finalize(user, event).await?;
                Ok(Drive::Confirmed)
            }
            TxStatus::Failed(reason) => {
                asp_events::mark_failed(&self.db, event.id, &reason).await?;
                Ok(Drive::Failed)
            }
            // Past its time bound the network can no longer include it.
            TxStatus::NotFound if self.clock.now() > event.valid_until => {
                asp_events::mark_failed(&self.db, event.id, "expired before inclusion").await?;
                Ok(Drive::Failed)
            }
            TxStatus::NotFound | TxStatus::Pending => Ok(Drive::Unsettled),
        }
    }

    /// Marks the event confirmed and moves the user's status, atomically.
    async fn finalize(
        &self,
        user: &ShieldUser,
        event: &SubmittedEvent,
    ) -> Result<(), AspServiceError> {
        let mut tx = self.db.begin().await?;
        asp_events::mark_confirmed(&mut *tx, event.id).await?;
        // Onboarding may owe more actions (blocked scenario: allow, then
        // block); admin actions on a settled user owe none.
        let remaining = if user.asp_status == AspStatus::Pending {
            let confirmed = asp_events::confirmed_actions(&mut *tx, user.id).await?;
            rules::remaining_onboarding_actions(user.kyc_scenario, &confirmed).len()
        } else {
            0
        };
        let status = rules::status_after(event.action, remaining);
        users::set_asp_status(&mut *tx, user.id, status).await?;
        tx.commit().await?;
        tracing::info!(user_id = %user.id, action = %event.action, tx_hash = %event.tx_hash, %status, "ASP update confirmed");
        Ok(())
    }

    async fn find(&self, account: &AccountId) -> Result<ShieldUser, AspServiceError> {
        users::find_by_account(&self.db, account)
            .await?
            .ok_or(AspServiceError::UserNotFound)
    }

    async fn view(&self, account: &AccountId) -> Result<UserView, AspServiceError> {
        let user = self.find(account).await?;
        let last_asp_tx_hash = asp_events::last_tx_hash(&self.db, user.id).await?;
        Ok(UserView {
            user,
            last_asp_tx_hash,
        })
    }
}
