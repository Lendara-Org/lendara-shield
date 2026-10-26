//! `AspService` against a real Postgres and the in-memory fake chain:
//! scenarios, idempotency, resumption and every chain failure mode.

use std::{sync::Arc, time::Duration};

use chrono::{TimeZone, Utc};
use shield_api::{
    asp::{
        AspAction, AspStatus, KycScenario, KycStatus,
        service::{AspService, AspServiceError, Onboarded, OnboardingInput, UserView},
    },
    domain::{AccountId, PoolField},
    testing::{AppliedWrite, ChainBehaviour, FakeAspContract, FakeClock},
};
use sqlx::PgPool;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ALICE: &str = "GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC";
const BOB: &str = "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY";

struct Harness {
    service: AspService,
    chain: Arc<FakeAspContract>,
    clock: Arc<FakeClock>,
}

fn harness(db: PgPool) -> Harness {
    let now = Utc
        .with_ymd_and_hms(2026, 10, 26, 12, 0, 0)
        .single()
        .unwrap_or_default();
    let chain = Arc::new(FakeAspContract::default());
    chain.set_now(now);
    let clock = Arc::new(FakeClock::at(now));
    let service = AspService::new(db, chain.clone(), clock.clone())
        .with_timing(Duration::from_millis(60), Duration::from_millis(5));
    Harness {
        service,
        chain,
        clock,
    }
}

fn field(byte: u8) -> Result<PoolField, Box<dyn std::error::Error>> {
    Ok(PoolField::parse(&format!(
        "0x{}",
        format!("{byte:02x}").repeat(32)
    ))?)
}

/// Distinct keys per account: note key 0x11.., encryption 0x22.., leaf 0x33..
/// (offset by `seed`).
fn input(
    account: &str,
    seed: u8,
    scenario: KycScenario,
) -> Result<OnboardingInput, Box<dyn std::error::Error>> {
    Ok(OnboardingInput {
        stellar_account: AccountId::parse(account)?,
        note_public_key: field(0x10 + seed)?,
        encryption_public_key: field(0x20 + seed)?,
        asp_leaf: field(0x30 + seed)?,
        scenario,
    })
}

fn created(outcome: Onboarded) -> Result<UserView, String> {
    match outcome {
        Onboarded::Created(view) => Ok(view),
        Onboarded::Existing(_) => Err("expected a new user".into()),
    }
}

#[sqlx::test]
async fn approved_kyc_allow_lists_the_leaf(db: PgPool) -> TestResult {
    let h = harness(db);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Approved)?)
            .await?,
    )?;

    assert_eq!(view.user.kyc_status, KycStatus::Approved);
    assert_eq!(view.user.asp_status, AspStatus::Allowed);
    assert!(view.last_asp_tx_hash.is_some());
    assert_eq!(
        h.chain.applied(),
        vec![AppliedWrite {
            action: AspAction::Allow,
            key: field(0x31)?
        }]
    );
    Ok(())
}

#[sqlx::test]
async fn rejected_kyc_never_touches_the_chain(db: PgPool) -> TestResult {
    let h = harness(db);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Rejected)?)
            .await?,
    )?;

    assert_eq!(view.user.kyc_status, KycStatus::Rejected);
    assert_eq!(view.user.asp_status, AspStatus::NotAllowed);
    assert_eq!(view.last_asp_tx_hash, None);
    assert!(h.chain.applied().is_empty());
    assert_eq!(h.chain.submits(), 0);
    Ok(())
}

#[sqlx::test]
async fn blocked_scenario_allows_then_blocks_the_note_key(db: PgPool) -> TestResult {
    let h = harness(db);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Blocked)?)
            .await?,
    )?;

    assert_eq!(view.user.asp_status, AspStatus::Blocked);
    assert_eq!(
        h.chain.applied(),
        vec![
            AppliedWrite {
                action: AspAction::Allow,
                key: field(0x31)?
            },
            AppliedWrite {
                action: AspAction::Block,
                key: field(0x11)?
            },
        ]
    );
    Ok(())
}

#[sqlx::test]
async fn replaying_the_same_onboarding_is_idempotent(db: PgPool) -> TestResult {
    let h = harness(db);
    h.service
        .onboard(input(ALICE, 1, KycScenario::Approved)?)
        .await?;
    let again = h
        .service
        .onboard(input(ALICE, 1, KycScenario::Approved)?)
        .await?;

    let Onboarded::Existing(view) = again else {
        return Err("expected the existing user".into());
    };
    assert_eq!(view.user.asp_status, AspStatus::Allowed);
    assert_eq!(h.chain.submits(), 1, "no second transaction");
    Ok(())
}

#[sqlx::test]
async fn refuses_different_keys_for_an_onboarded_account(db: PgPool) -> TestResult {
    let h = harness(db);
    h.service
        .onboard(input(ALICE, 1, KycScenario::Approved)?)
        .await?;
    let err = h
        .service
        .onboard(input(ALICE, 2, KycScenario::Approved)?)
        .await;

    assert!(matches!(err, Err(AspServiceError::AccountAlreadyOnboarded)));
    Ok(())
}

#[sqlx::test]
async fn refuses_pool_keys_registered_to_another_account(db: PgPool) -> TestResult {
    let h = harness(db);
    h.service
        .onboard(input(ALICE, 1, KycScenario::Approved)?)
        .await?;
    let err = h
        .service
        .onboard(input(BOB, 1, KycScenario::Approved)?)
        .await;

    assert!(matches!(err, Err(AspServiceError::PoolKeyTaken)));
    Ok(())
}

#[sqlx::test]
async fn chain_down_leaves_the_user_pending_and_a_retry_resumes(db: PgPool) -> TestResult {
    let h = harness(db);
    h.chain.set_behaviour(ChainBehaviour::Down);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Blocked)?)
            .await?,
    )?;
    assert_eq!(view.user.asp_status, AspStatus::Pending);
    assert_eq!(view.last_asp_tx_hash, None);

    h.chain.set_behaviour(ChainBehaviour::Healthy);
    let again = h
        .service
        .onboard(input(ALICE, 1, KycScenario::Blocked)?)
        .await?;
    let Onboarded::Existing(view) = again else {
        return Err("expected the existing user".into());
    };
    assert_eq!(view.user.asp_status, AspStatus::Blocked);
    assert_eq!(h.chain.applied().len(), 2);
    Ok(())
}

#[sqlx::test]
async fn an_ambiguous_submit_is_never_resent(db: PgPool) -> TestResult {
    let h = harness(db);
    h.chain
        .set_behaviour(ChainBehaviour::SubmitTimesOutButLands);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Approved)?)
            .await?,
    )?;

    // The stored hash is checked on the network instead of resending.
    assert_eq!(view.user.asp_status, AspStatus::Allowed);
    assert_eq!(h.chain.submits(), 1);
    assert_eq!(h.chain.applied().len(), 1);
    Ok(())
}

#[sqlx::test]
async fn a_slow_transaction_is_picked_up_by_a_later_read(db: PgPool) -> TestResult {
    let h = harness(db);
    h.chain.set_behaviour(ChainBehaviour::NeverConfirms);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Approved)?)
            .await?,
    )?;
    assert_eq!(view.user.asp_status, AspStatus::Pending);
    let hash = view
        .last_asp_tx_hash
        .clone()
        .ok_or("hash must be stored before sending")?;

    let account = AccountId::parse(ALICE)?;
    assert_eq!(
        h.service.user(&account).await?.user.asp_status,
        AspStatus::Pending
    );

    h.chain.confirm_pending();
    let view = h.service.user(&account).await?;
    assert_eq!(view.user.asp_status, AspStatus::Allowed);
    assert_eq!(view.last_asp_tx_hash, Some(hash));
    assert_eq!(h.chain.submits(), 1, "reads never send");
    Ok(())
}

#[sqlx::test]
async fn a_dropped_transaction_is_replaced_only_after_its_time_bound(db: PgPool) -> TestResult {
    let h = harness(db);
    h.chain.set_behaviour(ChainBehaviour::Drops);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Approved)?)
            .await?,
    )?;
    assert_eq!(view.user.asp_status, AspStatus::Pending);
    assert_eq!(h.chain.submits(), 1);

    // Unknown to the network but still inside its time bound: it could
    // still land, so a retry must not build a second transaction.
    h.chain.set_behaviour(ChainBehaviour::Healthy);
    h.service
        .onboard(input(ALICE, 1, KycScenario::Approved)?)
        .await?;
    assert_eq!(h.chain.submits(), 1);

    // Past the time bound it can never land: now it is safe to replace.
    h.clock.advance(chrono::Duration::seconds(61));
    let again = h
        .service
        .onboard(input(ALICE, 1, KycScenario::Approved)?)
        .await?;
    let Onboarded::Existing(view) = again else {
        return Err("expected the existing user".into());
    };
    assert_eq!(view.user.asp_status, AspStatus::Allowed);
    assert_eq!(h.chain.submits(), 2);
    assert_eq!(h.chain.applied().len(), 1, "the leaf is inserted once");
    Ok(())
}

#[sqlx::test]
async fn a_chain_that_keeps_rejecting_does_not_loop(db: PgPool) -> TestResult {
    let h = harness(db);
    h.chain.set_behaviour(ChainBehaviour::Rejects);
    let view = created(
        h.service
            .onboard(input(ALICE, 1, KycScenario::Approved)?)
            .await?,
    )?;

    assert_eq!(view.user.asp_status, AspStatus::Pending);
    assert_eq!(h.chain.submits(), 3, "bounded attempts per request");
    assert_eq!(
        view.last_asp_tx_hash, None,
        "failed transactions are not reported"
    );
    Ok(())
}

#[sqlx::test]
async fn admin_can_block_and_unblock(db: PgPool) -> TestResult {
    let h = harness(db);
    h.service
        .onboard(input(ALICE, 1, KycScenario::Approved)?)
        .await?;
    let account = AccountId::parse(ALICE)?;

    assert_eq!(
        h.service.block(&account).await?.user.asp_status,
        AspStatus::Blocked
    );
    assert_eq!(
        h.service.unblock(&account).await?.user.asp_status,
        AspStatus::Allowed
    );
    assert_eq!(
        h.chain.applied(),
        vec![
            AppliedWrite {
                action: AspAction::Allow,
                key: field(0x31)?
            },
            AppliedWrite {
                action: AspAction::Block,
                key: field(0x11)?
            },
            AppliedWrite {
                action: AspAction::Unblock,
                key: field(0x11)?
            },
        ]
    );
    Ok(())
}

#[sqlx::test]
async fn admin_actions_respect_the_transition_rules(db: PgPool) -> TestResult {
    let h = harness(db);
    h.service
        .onboard(input(ALICE, 1, KycScenario::Rejected)?)
        .await?;
    h.chain.set_behaviour(ChainBehaviour::Down);
    h.service
        .onboard(input(BOB, 2, KycScenario::Approved)?)
        .await?;
    let alice = AccountId::parse(ALICE)?;
    let bob = AccountId::parse(BOB)?;

    // not_allowed is final; pending belongs to onboarding.
    assert!(matches!(
        h.service.block(&alice).await,
        Err(AspServiceError::Transition(_))
    ));
    assert!(matches!(
        h.service.block(&bob).await,
        Err(AspServiceError::Transition(_))
    ));
    assert!(matches!(
        h.service.unblock(&bob).await,
        Err(AspServiceError::Transition(_))
    ));
    assert_eq!(h.chain.submits(), 0);
    Ok(())
}

#[sqlx::test]
async fn unknown_users_are_not_found(db: PgPool) -> TestResult {
    let h = harness(db);
    let account = AccountId::parse(ALICE)?;

    assert!(matches!(
        h.service.user(&account).await,
        Err(AspServiceError::UserNotFound)
    ));
    assert!(matches!(
        h.service.block(&account).await,
        Err(AspServiceError::UserNotFound)
    ));
    Ok(())
}

#[sqlx::test]
async fn concurrent_onboardings_of_one_account_write_once(db: PgPool) -> TestResult {
    let h = harness(db);
    let (a, b) = tokio::join!(
        h.service.onboard(input(ALICE, 1, KycScenario::Approved)?),
        h.service.onboard(input(ALICE, 1, KycScenario::Approved)?),
    );
    let outcomes = [a?, b?];

    let created = outcomes
        .iter()
        .filter(|o| matches!(o, Onboarded::Created(_)))
        .count();
    assert_eq!(created, 1);
    assert_eq!(h.chain.applied().len(), 1, "the leaf is inserted once");
    Ok(())
}
