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
        anchor_sync::AnchorSync,
        matching::ExpectedAsset,
        service::{CreateIntent, IntentService},
        watcher::{IntakeWatcher, WatchReport},
    },
    ports::{AnchorClient, AnchorStatus, IncomingPayment},
    store::{
        intake_payments,
        intents::Intent,
        users::{self, NewUser},
    },
    testing::{AnchorBehaviour, FakeAnchor, FakeClock, FakeLedger},
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

#[sqlx::test]
async fn the_anchor_payment_funds_its_intent(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);

    assert_eq!(
        h.watcher.poll().await?,
        WatchReport {
            recorded: 1,
            funded: 1
        }
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
    h.ledger.push(payment(4, memo(&intent), "USDC", ISSUER)?); // funds it
    h.ledger.push(payment(5, memo(&intent), "USDC", ISSUER)?); // second payment

    assert_eq!(
        h.watcher.poll().await?,
        WatchReport {
            recorded: 5,
            funded: 1
        }
    );
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
            "intent is funds_received"
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
    assert_eq!(h.watcher.poll().await?.funded, 1);
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
    h.anchor.set_status(&id(&waiting), AnchorStatus::Completed);

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
        "completed still waits for the payment"
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
async fn a_payment_wins_over_a_late_expiry(db: PgPool) -> TestResult {
    let h = harness(db).await?;
    let intent = h.intent().await?;
    h.ledger.push(payment(1, memo(&intent), "USDC", ISSUER)?);
    h.watcher.poll().await?;

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
            funded: 0
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
    // The anchor already reports both as done; payments are read first, so
    // the paid one is funded, not expired.
    h.anchor.set_status(
        &paid.anchor_tx_id.clone().unwrap_or_default(),
        AnchorStatus::Expired,
    );
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
