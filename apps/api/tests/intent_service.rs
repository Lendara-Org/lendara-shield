//! `IntentService` and the intents store against a real Postgres and a fake
//! anchor.

use std::sync::Arc;

use shield_api::{
    asp::{AspStatus, KycScenario, KycStatus},
    domain::{AccountId, DecimalAmount, FiatCurrency, PoolField, TxHash},
    intent::{
        IntentStatus,
        service::{CreateIntent, IntentError, IntentService},
    },
    ports::AnchorClient,
    store::{
        intents::{self, Changes, TransitionError},
        users::{self, NewUser, ShieldUser},
    },
    testing::{AnchorBehaviour, FakeAnchor},
};
use sqlx::PgPool;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ALICE: &str = "GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC";
const BOB: &str = "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY";

async fn user(
    db: &PgPool,
    account: &str,
    seed: u8,
    status: AspStatus,
) -> Result<ShieldUser, Box<dyn std::error::Error>> {
    let field = |b: u8| PoolField::parse(&format!("0x{}", format!("{b:02x}").repeat(32)));
    Ok(users::insert(
        db,
        &NewUser {
            stellar_account: AccountId::parse(account)?,
            note_public_key: field(0x10 + seed)?,
            encryption_public_key: field(0x20 + seed)?,
            asp_leaf: field(0x05 + seed)?,
            kyc_scenario: KycScenario::Approved,
            kyc_status: if status == AspStatus::NotAllowed {
                KycStatus::Rejected
            } else {
                KycStatus::Approved
            },
            asp_status: status,
        },
    )
    .await?)
}

fn service(db: PgPool) -> (IntentService, Arc<FakeAnchor>) {
    let anchor = Arc::new(FakeAnchor::default());
    let service = IntentService::new(
        db,
        Some(anchor.clone() as Arc<dyn AnchorClient>),
        "USDC".into(),
    );
    (service, anchor)
}

fn request(account: &str, amount: &str) -> Result<CreateIntent, Box<dyn std::error::Error>> {
    Ok(CreateIntent {
        stellar_account: AccountId::parse(account)?,
        fiat_currency: FiatCurrency::parse("CRC")?,
        amount: DecimalAmount::parse(amount)?,
    })
}

#[sqlx::test]
async fn creates_an_intent_waiting_for_the_anchor(db: PgPool) -> TestResult {
    let alice = user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, anchor) = service(db.clone());

    let intent = service.create(request(ALICE, "5000.50")?).await?;

    assert_eq!(intent.status, IntentStatus::AwaitingAnchor);
    assert_eq!(intent.user_id, alice.id);
    assert_eq!(intent.amount_fiat.as_str(), "5000.5");
    assert_eq!(intent.asset_code, "USDC");
    assert_eq!(intent.anchor_home_domain, FakeAnchor::HOME_DOMAIN);
    assert!(intent.anchor_tx_id.is_some() && intent.interactive_url.is_some());
    assert_eq!(intent.amount_asset, None);

    let [sent] = anchor
        .requests()
        .try_into()
        .map_err(|_| "expected one anchor request")?;
    assert_eq!(sent.source_asset, "iso4217:CRC");
    assert_eq!(
        sent.customer_memo, alice.sep10_memo,
        "the user is the anchor's customer"
    );
    assert_eq!(
        sent.deposit_memo, intent.memo,
        "the payment is matched by the intent memo"
    );

    assert_eq!(
        intents::events(&db, intent.id).await?,
        vec![
            (None, IntentStatus::Created),
            (Some(IntentStatus::Created), IntentStatus::AwaitingAnchor),
        ]
    );
    Ok(())
}

#[sqlx::test]
async fn memos_are_unique_per_intent(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, _) = service(db);
    let first = service.create(request(ALICE, "10")?).await?;
    let second = service.create(request(ALICE, "10")?).await?;

    assert!(first.memo > 0);
    assert_ne!(first.memo, second.memo);
    Ok(())
}

#[sqlx::test]
async fn ramp_mode_can_be_disabled(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let service = IntentService::new(db.clone(), None, "USDC".into());

    assert!(matches!(
        service.create(request(ALICE, "10")?).await,
        Err(IntentError::RampDisabled)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM intents")
        .fetch_one(&db)
        .await?;
    assert_eq!(count, 0);
    Ok(())
}

#[sqlx::test]
async fn only_allowed_users_can_start_an_intent(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Pending).await?;
    user(&db, BOB, 2, AspStatus::Blocked).await?;
    let (service, anchor) = service(db);

    for account in [ALICE, BOB] {
        assert!(
            matches!(
                service.create(request(account, "10")?).await,
                Err(IntentError::UserNotAllowed)
            ),
            "{account}"
        );
    }
    assert!(
        anchor.requests().is_empty(),
        "the anchor is never contacted"
    );

    let unknown = "GAABX6R4F4UBYOHKFXFKKAWOGELDS3WFYGVQUYECOE5NTBYWY7J5WWMS";
    assert!(matches!(
        service.create(request(unknown, "10")?).await,
        Err(IntentError::UserNotFound)
    ));
    Ok(())
}

#[sqlx::test]
async fn an_anchor_failure_leaves_the_intent_in_error(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, anchor) = service(db.clone());

    for behaviour in [AnchorBehaviour::Down, AnchorBehaviour::Refuses] {
        anchor.set_behaviour(behaviour);
        let Err(IntentError::AnchorUnavailable { intent_id }) =
            service.create(request(ALICE, "10")?).await
        else {
            return Err(format!("{behaviour:?}: expected AnchorUnavailable").into());
        };
        let intent = service.get(intent_id).await?;
        assert_eq!(intent.status, IntentStatus::Error);
        assert!(
            intent.message.is_some_and(|m| !m.contains("connection")),
            "no internals leaked"
        );
        assert_eq!(
            intents::events(&db, intent_id).await?.last(),
            Some(&(Some(IntentStatus::Created), IntentStatus::Error))
        );
    }
    Ok(())
}

#[sqlx::test]
async fn unknown_intents_are_not_found(db: PgPool) -> TestResult {
    let (service, _) = service(db);
    assert!(matches!(
        service.get(Uuid::now_v7()).await,
        Err(IntentError::NotFound)
    ));
    Ok(())
}

#[sqlx::test]
async fn the_store_refuses_illegal_and_stale_transitions(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, _) = service(db.clone());
    let intent = service.create(request(ALICE, "10")?).await?;
    let mut conn = db.acquire().await?;

    // Illegal per the state machine: never reaches the database.
    let err = intents::transition(
        &mut conn,
        intent.id,
        IntentStatus::AwaitingAnchor,
        IntentStatus::Completed,
        Changes::default(),
    )
    .await;
    assert!(matches!(err, Err(TransitionError::Invalid(_))));

    // Legal, but the intent is not in `from` any more.
    let err = intents::transition(
        &mut conn,
        intent.id,
        IntentStatus::Created,
        IntentStatus::Error,
        Changes::default(),
    )
    .await;
    assert!(matches!(err, Err(TransitionError::Stale(id)) if id == intent.id));

    assert_eq!(
        service.get(intent.id).await?.status,
        IntentStatus::AwaitingAnchor
    );
    Ok(())
}

#[sqlx::test]
async fn walks_the_happy_path_and_stamps_completion(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, _) = service(db.clone());
    let intent = service.create(request(ALICE, "10")?).await?;
    let mut conn = db.acquire().await?;
    let hash = |b: u8| TxHash::from_bytes([b; 32]);
    let pool_tx = |b: u8| shield_api::ports::PoolTx {
        hash: hash(b),
        envelope_xdr: format!("envelope-{b}"),
        sequence: i64::from(b),
    };

    let steps = [
        (
            IntentStatus::AwaitingAnchor,
            IntentStatus::FundsReceived,
            Changes {
                amount_asset: Some(DecimalAmount::parse("9.9900000")?),
                intake_payment_tx_hash: Some(hash(1)),
                ..Changes::default()
            },
        ),
        // Each step stores the transaction it is about to send.
        (
            IntentStatus::FundsReceived,
            IntentStatus::Shielding,
            Changes {
                pool_deposit: Some(pool_tx(2)),
                ..Changes::default()
            },
        ),
        (
            IntentStatus::Shielding,
            IntentStatus::Transferring,
            Changes {
                pool_transfer: Some(pool_tx(3)),
                ..Changes::default()
            },
        ),
        (
            IntentStatus::Transferring,
            IntentStatus::Completed,
            Changes::default(),
        ),
    ];
    for (from, to, changes) in steps {
        intents::transition(&mut conn, intent.id, from, to, changes).await?;
    }

    let done = service.get(intent.id).await?;
    assert_eq!(done.status, IntentStatus::Completed);
    assert!(done.completed_at.is_some());
    assert_eq!(done.amount_asset.map(String::from).as_deref(), Some("9.99"));
    assert_eq!(done.intake_payment_tx_hash, Some(hash(1)));
    assert_eq!(done.pool_deposit, Some(pool_tx(2)));
    assert_eq!(done.pool_transfer, Some(pool_tx(3)));
    assert_eq!(intents::events(&db, intent.id).await?.len(), 6);
    Ok(())
}

#[sqlx::test]
async fn shielding_without_a_stored_deposit_is_refused(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, _) = service(db.clone());
    let intent = service.create(request(ALICE, "10")?).await?;
    let mut conn = db.acquire().await?;
    intents::transition(
        &mut conn,
        intent.id,
        IntentStatus::AwaitingAnchor,
        IntentStatus::FundsReceived,
        Changes::default(),
    )
    .await?;

    // The database itself refuses a `shielding` intent with no transaction.
    let err = intents::transition(
        &mut conn,
        intent.id,
        IntentStatus::FundsReceived,
        IntentStatus::Shielding,
        Changes::default(),
    )
    .await;
    assert!(err.is_err());
    assert_eq!(
        service.get(intent.id).await?.status,
        IntentStatus::FundsReceived
    );
    Ok(())
}

#[sqlx::test]
async fn one_anchor_payment_can_fund_only_one_intent(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, _) = service(db.clone());
    let a = service.create(request(ALICE, "10")?).await?;
    let b = service.create(request(ALICE, "10")?).await?;
    let mut conn = db.acquire().await?;
    let funded = |hash: TxHash| Changes {
        intake_payment_tx_hash: Some(hash),
        ..Changes::default()
    };

    intents::transition(
        &mut conn,
        a.id,
        IntentStatus::AwaitingAnchor,
        IntentStatus::FundsReceived,
        funded(TxHash::from_bytes([7; 32])),
    )
    .await?;
    let reused = intents::transition(
        &mut conn,
        b.id,
        IntentStatus::AwaitingAnchor,
        IntentStatus::FundsReceived,
        funded(TxHash::from_bytes([7; 32])),
    )
    .await;

    assert!(
        reused.is_err(),
        "the unique constraint stops a double shield"
    );
    assert_eq!(
        service.get(b.id).await?.status,
        IntentStatus::AwaitingAnchor
    );
    Ok(())
}

#[sqlx::test]
async fn concurrent_transitions_of_one_intent_have_one_winner(db: PgPool) -> TestResult {
    user(&db, ALICE, 1, AspStatus::Allowed).await?;
    let (service, _) = service(db.clone());
    let intent = service.create(request(ALICE, "10")?).await?;
    let (mut c1, mut c2) = (db.acquire().await?, db.acquire().await?);

    let (r1, r2) = tokio::join!(
        intents::transition(
            &mut c1,
            intent.id,
            IntentStatus::AwaitingAnchor,
            IntentStatus::Expired,
            Changes::default()
        ),
        intents::transition(
            &mut c2,
            intent.id,
            IntentStatus::AwaitingAnchor,
            IntentStatus::Error,
            Changes::default()
        ),
    );

    assert_eq!([r1.is_ok(), r2.is_ok()].iter().filter(|ok| **ok).count(), 1);
    assert_eq!(
        intents::events(&db, intent.id).await?.len(),
        3,
        "one event per applied change"
    );
    Ok(())
}
