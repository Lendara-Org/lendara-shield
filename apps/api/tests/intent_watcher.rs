//! The intake watcher and the anchor status sync against a real Postgres,
//! a fake ledger and a fake anchor.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use shield_api::{
    asp::{AspStatus, KycScenario, KycStatus},
    background::Background,
    domain::{AccountId, DecimalAmount, FiatCurrency, PoolField, TxHash},
    intent::{
        IntentStatus,
        anchor_sync::{ANCHOR_PAYMENT_GRACE, AnchorSync, UNCONFIRMED_PAYMENT_MESSAGE},
        matching::ExpectedAsset,
        service::{CreateIntent, IntentService},
        shielding::ShieldingWorker,
        watcher::{IntakeWatcher, WatchReport},
    },
    ports::{AnchorClient, AnchorStatus, IncomingPayment},
    store::{
        intake_payments,
        intents::Intent,
        users::{self, NewUser},
    },
    testing::{AnchorBehaviour, FakeAnchor, FakeClock, FakeLedger, FakePool, PoolBehaviour},
};
use sqlx::PgPool;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ALICE: &str = "GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC";
const ANCHOR_ACCOUNT: &str = "GAO7YFNVZAHK6RSKV2LNPIS3WDGCHLBRLIFQETTPE36TLFFAGENNMNT5";
const ISSUER: &str = "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5";

struct Harness {
    db: PgPool,
    intents: IntentService,
    anchor: Arc<FakeAnchor>,
    ledger: Arc<FakeLedger>,
    watcher: IntakeWatcher,
    sync: AnchorSync,
    clock: Arc<FakeClock>,
}

async fn harness(db: PgPool) -> Result<Harness, Box<dyn std::error::Error>> {
    let field = |b: u8| PoolField::parse(&format!("0x{}", format!("{b:02x}").repeat(32)));
    users::insert(
        &db,
        &NewUser {
            stellar_account: AccountId::parse(ALICE)?,
            note_public_key: field(0x11)?,
            encryption_public_key: field(0x21)?,
            asp_leaf: field(0x06)?,
            kyc_scenario: KycScenario::Approved,
            kyc_status: KycStatus::Approved,
            asp_status: AspStatus::Allowed,
        },
    )
    .await?;
    let anchor = Arc::new(FakeAnchor::default());
    let ledger = Arc::new(FakeLedger::default());
    let clock = Arc::new(FakeClock::at(
        Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0)
            .single()
            .ok_or("time")?,
    ));
    let asset = ExpectedAsset {
        code: "USDC".into(),
        issuer: ISSUER.into(),
    };
    Ok(Harness {
        intents: IntentService::new(
            db.clone(),
            Some(anchor.clone() as Arc<dyn AnchorClient>),
            "USDC".into(),
        ),
        watcher: IntakeWatcher::new(db.clone(), ledger.clone(), asset),
        sync: AnchorSync::new(db.clone(), anchor.clone(), clock.clone()),
        db,
        anchor,
        ledger,
        clock,
    })
}

impl Harness {
    async fn intent(&self) -> Result<Intent, Box<dyn std::error::Error>> {
        Ok(self
            .intents
            .create(CreateIntent {
                request_id: uuid::Uuid::now_v7(),
                stellar_account: AccountId::parse(ALICE)?,
                fiat_currency: FiatCurrency::parse("CRC")?,
                amount: DecimalAmount::parse("5000")?,
            })
            .await?)
    }
}

fn payment(
    op: u8,
    memo: Option<u64>,
    code: &str,
    issuer: &str,
) -> Result<IncomingPayment, Box<dyn std::error::Error>> {
    Ok(IncomingPayment {
        operation_id: format!("op-{op}"),
        tx_hash: TxHash::from_bytes([op; 32]),
        from: AccountId::parse(ANCHOR_ACCOUNT)?,
        asset_code: code.into(),
        asset_issuer: issuer.into(),
        amount: DecimalAmount::parse("9.3039215")?,
        memo,
    })
}

fn memo(intent: &Intent) -> Option<u64> {
    u64::try_from(intent.memo).ok()
}

impl Harness {
    /// The anchor reports the deposit completed in transaction `op`.
    fn anchor_paid(&self, intent: &Intent, op: u8) {
        self.anchor.set_status(
            &intent.anchor_tx_id.clone().unwrap_or_default(),
            AnchorStatus::Completed(Some(TxHash::from_bytes([op; 32]))),
        );
    }

    async fn status(&self, intent: &Intent) -> Result<IntentStatus, Box<dyn std::error::Error>> {
        Ok(self.intents.get(intent.id).await?.status)
    }
}

/// A payment with the right asset and memo from someone who is not the anchor.
fn dust(op: u8, memo: Option<u64>) -> Result<IncomingPayment, Box<dyn std::error::Error>> {
    Ok(IncomingPayment {
        from: AccountId::parse(ALICE)?,
        amount: DecimalAmount::parse("0.0000001")?,
        ..payment(op, memo, "USDC", ISSUER)?
    })
}

#[sqlx::test]
async fn the_payment_the_anchor_confirms_funds_its_intent(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);

    assert_eq!(
        h.watcher.poll().await?,
        WatchReport {
            recorded: 1,
            matched: 1
        }
    );
    assert_eq!(
        h.status(&intent).await?,
        IntentStatus::AwaitingAnchor,
        "a memo alone funds nothing"
    );

    h.anchor_paid(&intent, 1);
    assert_eq!(
        h.sync.check_due().await?,
        1,
        "checked right away, not in 20 s"
    );

    let funded = h.intents.get(intent.id).await?;
    assert_eq!(funded.status, IntentStatus::FundsReceived);
    assert_eq!(
        funded.amount_asset.map(String::from).as_deref(),
        Some("9.3039215")
    );
    assert_eq!(
        funded.intake_payment_tx_hash,
        Some(TxHash::from_bytes([1; 32]))
    );
    assert_eq!(intake_payments::unmatched_count(&h.db).await?, 0);
    Ok(())
}

#[sqlx::test]
async fn polling_again_is_a_no_op(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);
    h.watcher.poll().await?;

    assert_eq!(
        h.watcher.poll().await?,
        WatchReport::default(),
        "the cursor moved past it"
    );

    // Even if the cursor were lost, the payment is not applied twice.
    sqlx::query("DELETE FROM watcher_cursors")
        .execute(&h.db)
        .await?;
    assert_eq!(h.watcher.poll().await?, WatchReport::default());
    Ok(())
}

#[sqlx::test]
async fn unmatched_payments_are_kept_for_review(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger
        .push(payment(1, memo(&intent), "USDC", ANCHOR_ACCOUNT)?); // fake USDC
    h.ledger.push(payment(2, None, "USDC", ISSUER)?); // no memo
    h.ledger.push(payment(3, Some(999_999), "USDC", ISSUER)?); // unknown intent
    h.ledger.push(payment(4, memo(&intent), "USDC", ISSUER)?); // the anchor's
    h.ledger.push(payment(5, memo(&intent), "USDC", ISSUER)?); // someone else's

    assert_eq!(
        h.watcher.poll().await?,
        WatchReport {
            recorded: 5,
            matched: 2
        }
    );
    h.anchor_paid(&intent, 4);
    h.sync.check_due().await?;
    assert_eq!(
        h.intents.get(intent.id).await?.intake_payment_tx_hash,
        Some(TxHash::from_bytes([4; 32]))
    );

    let reasons: Vec<String> = sqlx::query_scalar(
        "SELECT unmatched_reason FROM intake_payments WHERE intent_id IS NULL ORDER BY operation_id",
    )
    .fetch_all(&h.db)
    .await?;
    assert_eq!(
        reasons,
        [
            "wrong asset",
            "no id memo",
            "no intent with this memo",
            intake_payments::UNCONFIRMED,
        ]
    );
    Ok(())
}

#[sqlx::test]
async fn reads_many_pages(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    for op in 0..=250u8 {
        h.ledger.push(payment(
            op,
            Some(1_000_000 + u64::from(op)),
            "USDC",
            ISSUER,
        )?);
    }
    let report = h.watcher.poll().await?;
    assert_eq!(report.recorded, 251, "three pages of 100");
    Ok(())
}

#[sqlx::test]
async fn a_ledger_outage_is_reported_and_retried(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);
    h.ledger.set_down(true);
    assert!(h.watcher.poll().await.is_err());

    h.ledger.set_down(false);
    assert_eq!(h.watcher.poll().await?.matched, 1);
    Ok(())
}

#[sqlx::test]
async fn the_anchor_can_expire_or_fail_a_deposit(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let expired = h.intent().await?;
    let failed = h.intent().await?;
    let waiting = h.intent().await?;
    let id = |i: &Intent| i.anchor_tx_id.clone().unwrap_or_default();
    h.anchor.set_status(&id(&expired), AnchorStatus::Expired);
    h.anchor.set_status(
        &id(&failed),
        AnchorStatus::Failed("KYC rejected (sandbox scenario).".into()),
    );
    h.anchor.set_status(
        &id(&waiting),
        AnchorStatus::Completed(Some(TxHash::from_bytes([9; 32]))),
    );

    assert_eq!(h.sync.check_due().await?, 2);

    assert_eq!(
        h.intents.get(expired.id).await?.status,
        IntentStatus::Expired
    );
    let failed = h.intents.get(failed.id).await?;
    assert_eq!(failed.status, IntentStatus::Error);
    assert_eq!(
        failed.message.as_deref(),
        Some("Anchor: KYC rejected (sandbox scenario).")
    );
    assert_eq!(
        h.intents.get(waiting.id).await?.status,
        IntentStatus::AwaitingAnchor,
        "completed still waits until Shield reads that payment"
    );

    // It authenticates as each user's customer at the anchor.
    let alice = users::find_by_account(&h.db, &AccountId::parse(ALICE)?)
        .await?
        .ok_or("alice")?;
    assert!(
        h.anchor
            .status_queries()
            .iter()
            .all(|(memo, _)| *memo == alice.sep10_memo)
    );
    Ok(())
}

#[sqlx::test]
async fn checks_are_throttled_per_intent(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    h.intent().await?;

    h.sync.check_due().await?;
    h.sync.check_due().await?;
    assert_eq!(h.anchor.status_queries().len(), 1, "not due again yet");

    h.clock.advance(chrono::Duration::seconds(21));
    h.sync.check_due().await?;
    assert_eq!(h.anchor.status_queries().len(), 2);
    Ok(())
}

#[sqlx::test]
async fn an_anchor_outage_keeps_the_intent_waiting(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.anchor.set_behaviour(AnchorBehaviour::Down);

    assert_eq!(h.sync.check_due().await?, 0);
    assert_eq!(
        h.intents.get(intent.id).await?.status,
        IntentStatus::AwaitingAnchor
    );
    Ok(())
}

#[sqlx::test]
async fn a_funded_intent_ignores_a_late_expiry(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);
    h.watcher.poll().await?;
    h.anchor_paid(&intent, 1);
    h.sync.check_due().await?;

    // The anchor flips to expired afterwards: the funded intent is untouched.
    h.anchor.set_status(
        &intent.anchor_tx_id.clone().unwrap_or_default(),
        AnchorStatus::Expired,
    );
    h.clock.advance(chrono::Duration::minutes(5));
    assert_eq!(h.sync.check_due().await?, 0);
    assert_eq!(
        h.intents.get(intent.id).await?.status,
        IntentStatus::FundsReceived
    );
    Ok(())
}

#[sqlx::test]
async fn a_payment_after_expiry_needs_review(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.anchor.set_status(
        &intent.anchor_tx_id.clone().unwrap_or_default(),
        AnchorStatus::Expired,
    );
    h.sync.check_due().await?;

    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);
    assert_eq!(
        h.watcher.poll().await?,
        WatchReport {
            recorded: 1,
            matched: 0
        }
    );
    assert_eq!(
        intake_payments::unmatched_count(&h.db).await?,
        1,
        "money in custody, flagged"
    );
    Ok(())
}

#[sqlx::test]
async fn a_background_round_funds_paid_intents_and_ends_expired_ones(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let paid = h.intent().await?;
    let unpaid = h.intent().await?;
    h.ledger.push(payment(1, memo(&paid), "USDC", ISSUER)?);
    // One round reads the payment and confirms it with the anchor.
    h.anchor_paid(&paid, 1);
    h.anchor.set_status(
        &unpaid.anchor_tx_id.clone().unwrap_or_default(),
        AnchorStatus::Expired,
    );

    Background {
        watcher: h.watcher.clone(),
        anchor_sync: h.sync.clone(),
        shielding: None,
    }
    .round()
    .await;

    assert_eq!(
        h.intents.get(paid.id).await?.status,
        IntentStatus::FundsReceived
    );
    assert_eq!(
        h.intents.get(unpaid.id).await?.status,
        IntentStatus::Expired
    );
    Ok(())
}

#[sqlx::test]
async fn the_loop_keeps_running_through_failures_and_stops_on_shutdown(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.set_down(true);
    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);
    h.anchor_paid(&intent, 1);
    let shutdown = tokio_util::sync::CancellationToken::new();
    let task = tokio::spawn(
        Background {
            watcher: h.watcher.clone(),
            anchor_sync: h.sync.clone(),
            shielding: None,
        }
        .run(std::time::Duration::from_millis(20), shutdown.clone()),
    );

    // Rounds fail while Horizon is down, then recover on their own.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    h.ledger.set_down(false);
    for _ in 0..50 {
        if h.intents.get(intent.id).await?.status == IntentStatus::FundsReceived {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(
        h.intents.get(intent.id).await?.status,
        IntentStatus::FundsReceived
    );

    shutdown.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(2), task).await??;
    Ok(())
}

#[sqlx::test]
async fn dust_with_the_memo_cannot_hijack_a_deposit(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    // An attacker front-runs the anchor with the intent's memo.
    h.ledger.push(dust(7, memo(&intent))?);
    h.ledger.push(payment(8, memo(&intent), "USDC", ISSUER)?);
    h.watcher.poll().await?;
    h.anchor_paid(&intent, 8);
    h.sync.check_due().await?;

    let funded = h.intents.get(intent.id).await?;
    assert_eq!(funded.status, IntentStatus::FundsReceived);
    assert_eq!(
        funded.amount_asset.map(String::from).as_deref(),
        Some("9.3039215")
    );
    assert_eq!(
        funded.intake_payment_tx_hash,
        Some(TxHash::from_bytes([8; 32]))
    );
    assert_eq!(
        intake_payments::unmatched_count(&h.db).await?,
        1,
        "the dust waits for review"
    );
    Ok(())
}

#[sqlx::test]
async fn a_payment_the_anchor_never_claims_funds_nothing(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.push(dust(7, memo(&intent))?);
    h.watcher.poll().await?;
    h.sync.check_due().await?; // the anchor still says pending

    // Completed without naming its transaction: nothing can be confirmed.
    h.anchor.set_status(
        &intent.anchor_tx_id.clone().unwrap_or_default(),
        AnchorStatus::Completed(None),
    );
    h.clock.advance(chrono::Duration::minutes(1));
    h.sync.check_due().await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::AwaitingAnchor);
    Ok(())
}

#[sqlx::test]
async fn an_anchor_report_ahead_of_horizon_waits_for_the_payment(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.anchor_paid(&intent, 3);
    assert_eq!(
        h.sync.check_due().await?,
        0,
        "Shield has not read the payment yet"
    );

    h.ledger.push(payment(3, memo(&intent), "USDC", ISSUER)?);
    h.watcher.poll().await?;
    h.clock.advance(chrono::Duration::seconds(5));
    assert_eq!(h.sync.check_due().await?, 1);
    assert_eq!(h.status(&intent).await?, IntentStatus::FundsReceived);
    Ok(())
}

#[sqlx::test]
async fn intent_memos_are_not_guessable(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let (a, b) = (h.intent().await?, h.intent().await?);
    // A sequence would give 1 and 2; random 63-bit memos are almost never
    // below 2^40 (probability 2^-23 each).
    assert!(
        a.memo > 1 << 40 && b.memo > 1 << 40,
        "{} {}",
        a.memo,
        b.memo
    );
    assert_ne!(b.memo, a.memo + 1);
    Ok(())
}

#[sqlx::test]
async fn an_anchor_payment_that_never_shows_up_ends_in_review(db: PgPool) -> TestResult {
    let h = harness(db.clone()).await?;
    let intent = h.intent().await?;
    // The anchor names transaction 3, which never reaches the intake.
    h.anchor_paid(&intent, 3);
    assert_eq!(h.sync.check_due().await?, 0, "the grace period starts");

    let grace = chrono::Duration::from_std(ANCHOR_PAYMENT_GRACE)?;
    let step = chrono::Duration::minutes(4);
    let mut waited = chrono::Duration::zero();
    while waited + step < grace {
        h.clock.advance(step);
        waited += step;
        assert_eq!(h.sync.check_due().await?, 0);
        assert_eq!(h.status(&intent).await?, IntentStatus::AwaitingAnchor);
    }
    h.clock.advance(step);
    assert_eq!(h.sync.check_due().await?, 1);
    let ended = h.intents.get(intent.id).await?;
    assert_eq!(ended.status, IntentStatus::Error);
    assert_eq!(ended.message.as_deref(), Some(UNCONFIRMED_PAYMENT_MESSAGE));
    assert!(ended.amount_asset.is_none(), "nothing was funded");

    // If the payment turns up later, it is kept for review and funds nothing.
    h.ledger.push(payment(3, memo(&intent), "USDC", ISSUER)?);
    h.watcher.poll().await?;
    let reason: Option<String> = sqlx::query_scalar(
        "SELECT unmatched_reason FROM intake_payments WHERE operation_id = 'op-3'",
    )
    .fetch_one(&db)
    .await?;
    assert_eq!(reason.as_deref(), Some("intent is error"));
    assert_eq!(h.status(&intent).await?, IntentStatus::Error);
    Ok(())
}

#[sqlx::test]
async fn a_completed_report_without_its_transaction_ends_in_review(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.anchor.set_status(
        &intent.anchor_tx_id.clone().unwrap_or_default(),
        AnchorStatus::Completed(None),
    );
    h.sync.check_due().await?;
    h.clock
        .advance(chrono::Duration::from_std(ANCHOR_PAYMENT_GRACE)? - chrono::Duration::seconds(1));
    h.sync.check_due().await?;
    assert_eq!(h.status(&intent).await?, IntentStatus::AwaitingAnchor);

    h.clock.advance(chrono::Duration::seconds(30));
    assert_eq!(h.sync.check_due().await?, 1);
    assert_eq!(h.status(&intent).await?, IntentStatus::Error);
    Ok(())
}

#[sqlx::test]
async fn a_payment_inside_the_grace_period_still_funds_the_intent(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.anchor_paid(&intent, 3);
    h.sync.check_due().await?;
    h.clock.advance(chrono::Duration::minutes(10));
    h.sync.check_due().await?;

    h.ledger.push(payment(3, memo(&intent), "USDC", ISSUER)?);
    h.watcher.poll().await?;
    h.clock.advance(chrono::Duration::seconds(5));
    assert_eq!(h.sync.check_due().await?, 1);
    assert_eq!(h.status(&intent).await?, IntentStatus::FundsReceived);
    Ok(())
}

#[sqlx::test]
async fn a_stuck_prover_holds_up_neither_payments_nor_the_anchor(db: PgPool) -> TestResult {
    let h = harness(db.clone()).await?;
    // One intent already funded: the worker hangs proving its deposit.
    let stuck = h.intent().await?;
    h.ledger.push(payment(1, memo(&stuck), "USDC", ISSUER)?);
    h.anchor_paid(&stuck, 1);
    h.watcher.poll().await?;
    h.sync.check_due().await?;
    assert_eq!(h.status(&stuck).await?, IntentStatus::FundsReceived);
    let waiting = h.intent().await?;

    let pool = Arc::new(FakePool::default());
    pool.set_behaviour(PoolBehaviour::ProverHangs);
    let shutdown = tokio_util::sync::CancellationToken::new();
    let task = tokio::spawn(
        Background {
            watcher: h.watcher.clone(),
            anchor_sync: h.sync.clone(),
            shielding: Some(ShieldingWorker::new(db, pool.clone(), h.clock.clone())),
        }
        .run(std::time::Duration::from_millis(20), shutdown.clone()),
    );

    // The worker is stuck on the first intent; only now does the second
    // one's payment arrive, so whatever order a single loop used, it would
    // never get to read it.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    h.ledger.push(payment(2, memo(&waiting), "USDC", ISSUER)?);
    h.anchor_paid(&waiting, 2);
    for _ in 0..50 {
        if h.status(&waiting).await? == IntentStatus::FundsReceived {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(h.status(&waiting).await?, IntentStatus::FundsReceived);
    assert_eq!(h.status(&stuck).await?, IntentStatus::FundsReceived);
    assert!(pool.applied().is_empty(), "the prover never finished");

    // Shutdown does not wait for the hung proof either.
    shutdown.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(2), task).await??;
    Ok(())
}
