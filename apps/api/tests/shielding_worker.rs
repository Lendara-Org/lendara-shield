//! The shielding worker against a real Postgres and a fake pool chain that
//! enforces the sequence rule. Every test ends by checking the invariant
//! that matters: one deposit and one transfer, never more.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use shield_api::{
    asp::{AspStatus, KycScenario, KycStatus},
    domain::{AccountId, DecimalAmount, FiatCurrency, PoolField, TxHash},
    intent::{
        IntentStatus,
        service::{CreateIntent, IntentService},
        shielding::{BLOCKED_USER_MESSAGE, MAX_ATTEMPTS, ShieldingWorker},
    },
    ports::AnchorClient,
    store::{
        intents::{self, Changes, Intent},
        users::{self, NewUser},
    },
    testing::{FakeAnchor, FakeClock, FakePool, PoolBehaviour, PoolWrite},
};
use sqlx::PgPool;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ALICE: &str = "GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC";
const AMOUNT: i64 = 93_039_215; // 9.3039215 USDC

struct Harness {
    db: PgPool,
    intents: IntentService,
    pool: Arc<FakePool>,
    clock: Arc<FakeClock>,
    worker: ShieldingWorker,
    note_key: PoolField,
}

/// A user and one intent already in `funds_received`.
async fn harness(db: PgPool) -> Result<(Harness, Intent), Box<dyn std::error::Error>> {
    let field = |b: u8| PoolField::parse(&format!("0x{}", format!("{b:02x}").repeat(32)));
    let note_key = field(0x11)?;
    users::insert(
        &db,
        &NewUser {
            stellar_account: AccountId::parse(ALICE)?,
            note_public_key: note_key.clone(),
            encryption_public_key: field(0x21)?,
            asp_leaf: field(0x06)?,
            kyc_scenario: KycScenario::Approved,
            kyc_status: KycStatus::Approved,
            asp_status: AspStatus::Allowed,
        },
    )
    .await?;
    let anchor: Arc<dyn AnchorClient> = Arc::new(FakeAnchor::default());
    let service = IntentService::new(db.clone(), Some(anchor), "USDC".into());
    let intent = service
        .create(CreateIntent {
            request_id: uuid::Uuid::now_v7(),
            stellar_account: AccountId::parse(ALICE)?,
            fiat_currency: FiatCurrency::parse("CRC")?,
            amount: DecimalAmount::parse("5000")?,
        })
        .await?;
    let mut conn = db.acquire().await?;
    let funded = intents::transition(
        &mut conn,
        intent.id,
        IntentStatus::AwaitingAnchor,
        IntentStatus::FundsReceived,
        Changes {
            amount_asset: Some(DecimalAmount::parse("9.3039215")?),
            intake_payment_tx_hash: Some(TxHash::from_bytes([0xee; 32])),
            ..Changes::default()
        },
    )
    .await?;

    let pool = Arc::new(FakePool::default());
    let clock = Arc::new(FakeClock::at(
        Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0)
            .single()
            .ok_or("time")?,
    ));
    Ok((
        Harness {
            worker: ShieldingWorker::new(db.clone(), pool.clone(), clock.clone()),
            intents: service,
            db,
            pool,
            clock,
            note_key,
        },
        funded,
    ))
}

impl Harness {
    /// Runs `n` worker rounds, `step` apart.
    async fn rounds(
        &self,
        n: usize,
        step: chrono::Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..n {
            self.clock.advance(step);
            self.worker.process_due().await?;
        }
        Ok(())
    }

    async fn status(&self, intent: &Intent) -> Result<IntentStatus, Box<dyn std::error::Error>> {
        Ok(self.intents.get(intent.id).await?.status)
    }

    /// The invariant: exactly one deposit and one transfer of the full
    /// amount, the transfer to the user's key.
    fn assert_paid_exactly_once(&self) {
        assert_eq!(
            self.pool.applied(),
            vec![
                PoolWrite::Deposit { amount: AMOUNT },
                PoolWrite::Transfer {
                    amount: AMOUNT,
                    note_public_key: self.note_key.clone()
                },
            ]
        );
    }
}

fn secs(n: i64) -> chrono::Duration {
    chrono::Duration::seconds(n)
}

#[sqlx::test]
async fn shields_and_transfers_to_completion(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.rounds(3, secs(10)).await?;

    let done = h.intents.get(intent.id).await?;
    assert_eq!(done.status, IntentStatus::Completed);
    assert!(done.completed_at.is_some());
    let (deposit, transfer) = (
        done.pool_deposit.ok_or("deposit")?,
        done.pool_transfer.ok_or("transfer")?,
    );
    assert_ne!(deposit.hash, transfer.hash);
    assert_eq!(
        intents::events(&h.db, intent.id)
            .await?
            .into_iter()
            .map(|(_, to)| to)
            .collect::<Vec<_>>(),
        [
            IntentStatus::Created,
            IntentStatus::AwaitingAnchor,
            IntentStatus::FundsReceived,
            IntentStatus::Shielding,
            IntentStatus::Transferring,
            IntentStatus::Completed,
        ]
    );
    h.assert_paid_exactly_once();
    assert_eq!(h.pool.prepares(), 2);
    Ok(())
}

#[sqlx::test]
async fn an_ambiguous_submit_never_pays_twice(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_behaviour(PoolBehaviour::SubmitTimesOutButLands);
    h.rounds(5, secs(10)).await?;

    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    assert_eq!(
        h.pool.prepares(),
        2,
        "the stored hash was checked, not re-prepared"
    );
    Ok(())
}

#[sqlx::test]
async fn a_dropped_transaction_is_resent_unchanged(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_behaviour(PoolBehaviour::Drops);
    h.rounds(1, secs(10)).await?;
    let stored = h
        .intents
        .get(intent.id)
        .await?
        .pool_deposit
        .ok_or("deposit stored before sending")?;

    h.pool.set_behaviour(PoolBehaviour::Healthy);
    h.rounds(4, secs(10)).await?;

    let done = h.intents.get(intent.id).await?;
    assert_eq!(done.status, IntentStatus::Completed);
    assert_eq!(
        done.pool_deposit.map(|tx| tx.hash),
        Some(stored.hash),
        "same transaction"
    );
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn a_transaction_whose_sequence_was_used_is_replaced(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_behaviour(PoolBehaviour::Drops);
    h.rounds(1, secs(10)).await?;
    let first = h
        .intents
        .get(intent.id)
        .await?
        .pool_deposit
        .ok_or("deposit")?;

    // Its sequence number is consumed elsewhere: it can never land now. It
    // is still only replaced after the grace period (it might be unindexed).
    h.pool.bump_sequence();
    h.pool.set_behaviour(PoolBehaviour::Healthy);
    h.rounds(2, secs(10)).await?;
    assert_eq!(h.pool.prepares(), 1, "not replaced within the grace period");
    h.rounds(6, secs(30)).await?;

    let done = h.intents.get(intent.id).await?;
    assert_eq!(done.status, IntentStatus::Completed);
    assert_ne!(done.pool_deposit.map(|tx| tx.hash), Some(first.hash));
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn an_on_chain_failure_is_replaced(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_behaviour(PoolBehaviour::FailsOnChain);
    h.rounds(1, secs(10)).await?;

    h.pool.set_behaviour(PoolBehaviour::Healthy);
    h.rounds(4, secs(10)).await?;

    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn a_refused_transaction_is_replaced(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_behaviour(PoolBehaviour::Drops);
    h.rounds(1, secs(10)).await?;
    // A stale proof (e.g. the block-list root changed) is refused outright.
    h.pool.set_behaviour(PoolBehaviour::Refuses);
    h.rounds(1, secs(10)).await?;

    h.pool.set_behaviour(PoolBehaviour::Healthy);
    h.rounds(5, secs(10)).await?;

    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn a_pending_transaction_is_waited_for(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_behaviour(PoolBehaviour::Pending);
    h.rounds(5, secs(10)).await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::Shielding);
    assert_eq!(h.pool.prepares(), 1, "no replacement while pending");

    h.pool.confirm_pending();
    h.pool.set_behaviour(PoolBehaviour::Healthy);
    h.rounds(3, secs(10)).await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn prepare_failures_back_off_and_recover(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_prepare_fails(Some(true));
    h.rounds(1, secs(10)).await?;
    // Backed off: an immediate second round does nothing.
    h.worker.process_due().await?;
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM intents WHERE id = $1")
        .bind(intent.id)
        .fetch_one(&h.db)
        .await?;
    assert_eq!(attempts, 1);

    h.pool.set_prepare_fails(None);
    h.rounds(4, secs(60)).await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn a_wallet_behind_the_chain_is_waited_for_without_spending_attempts(
    db: PgPool,
) -> TestResult {
    let (h, intent) = harness(db).await?;
    // More sync waits than MAX_ATTEMPTS: counting them would end in `error`.
    let waits = usize::try_from(MAX_ATTEMPTS)? * 2;
    h.pool.set_sync_required(waits);
    h.rounds(waits, secs(5)).await?;
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM intents WHERE id = $1")
        .bind(intent.id)
        .fetch_one(&h.db)
        .await?;
    assert_eq!(attempts, 0, "sync waits are not attempts");
    assert_eq!(h.status(&intent).await?, IntentStatus::FundsReceived);

    h.rounds(4, secs(60)).await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn gives_up_after_bounded_attempts_and_flags_for_review(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.pool.set_prepare_fails(Some(false));
    h.rounds(
        usize::try_from(MAX_ATTEMPTS)? + 2,
        chrono::Duration::minutes(10),
    )
    .await?;

    let failed = h.intents.get(intent.id).await?;
    assert_eq!(failed.status, IntentStatus::Error);
    assert!(failed.message.is_some_and(|m| m.contains("custody")));
    assert!(h.pool.applied().is_empty(), "nothing was paid");
    Ok(())
}

#[sqlx::test]
async fn survives_a_chaotic_network_and_pays_exactly_once(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    let script = [
        PoolBehaviour::Drops,
        PoolBehaviour::SubmitTimesOutButLands,
        PoolBehaviour::Pending,
        PoolBehaviour::Refuses,
        PoolBehaviour::Drops,
        PoolBehaviour::FailsOnChain,
        PoolBehaviour::SubmitTimesOutButLands,
    ];
    for (i, behaviour) in script.iter().enumerate() {
        h.pool.set_behaviour(*behaviour);
        if i == 3 {
            h.pool.confirm_pending();
        }
        h.rounds(1, secs(10)).await?;
    }
    h.pool.confirm_pending();
    h.pool.set_behaviour(PoolBehaviour::Healthy);
    h.rounds(8, secs(60)).await?;

    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn a_landed_transaction_not_yet_indexed_is_never_replaced(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    // The deposit lands, but the RPC does not report it for a while even
    // though the intake's sequence already moved past it.
    h.pool.set_status_lag(3);
    h.rounds(3, secs(10)).await?;
    assert_eq!(
        h.pool.prepares(),
        1,
        "no second deposit while it may have landed"
    );

    h.rounds(5, secs(30)).await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::Completed);
    h.assert_paid_exactly_once();
    Ok(())
}

#[sqlx::test]
async fn a_blocked_user_gets_no_deposit_and_the_funds_stay_in_custody(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    users::set_asp_status(&h.db, intent.user_id, AspStatus::Blocked).await?;
    h.rounds(3, secs(10)).await?;

    let stopped = h.intents.get(intent.id).await?;
    assert_eq!(stopped.status, IntentStatus::Error);
    assert_eq!(stopped.message.as_deref(), Some(BLOCKED_USER_MESSAGE));
    assert!(stopped.pool_deposit.is_none());
    assert!(h.pool.applied().is_empty());
    assert_eq!(h.pool.prepares(), 0);
    Ok(())
}

#[sqlx::test]
async fn a_user_blocked_after_the_deposit_never_receives_the_transfer(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.rounds(1, secs(10)).await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::Shielding);

    users::set_asp_status(&h.db, intent.user_id, AspStatus::Blocked).await?;
    h.rounds(3, secs(10)).await?;

    let stopped = h.intents.get(intent.id).await?;
    assert_eq!(stopped.status, IntentStatus::Error);
    assert_eq!(stopped.message.as_deref(), Some(BLOCKED_USER_MESSAGE));
    assert!(stopped.pool_transfer.is_none());
    // The deposit landed in Shield's own note; nothing went to the user.
    assert_eq!(
        h.pool.applied(),
        vec![PoolWrite::Deposit { amount: AMOUNT }]
    );
    assert_eq!(h.pool.prepares(), 1);
    Ok(())
}

#[sqlx::test]
async fn a_failed_transfer_is_not_rebuilt_for_a_user_blocked_meanwhile(db: PgPool) -> TestResult {
    let (h, intent) = harness(db).await?;
    h.rounds(1, secs(10)).await?; // deposit sent
    h.pool.set_behaviour(PoolBehaviour::FailsOnChain);
    h.rounds(1, secs(10)).await?; // deposit confirmed; the transfer fails on-chain
    assert_eq!(h.status(&intent).await?, IntentStatus::Transferring);

    users::set_asp_status(&h.db, intent.user_id, AspStatus::Blocked).await?;
    h.pool.set_behaviour(PoolBehaviour::Healthy);
    h.rounds(3, secs(10)).await?;

    let stopped = h.intents.get(intent.id).await?;
    assert_eq!(stopped.status, IntentStatus::Error);
    assert_eq!(stopped.message.as_deref(), Some(BLOCKED_USER_MESSAGE));
    assert_eq!(h.pool.prepares(), 2, "no second transfer was built");
    assert_eq!(
        h.pool.applied(),
        vec![PoolWrite::Deposit { amount: AMOUNT }]
    );
    Ok(())
}
