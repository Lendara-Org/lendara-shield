//! In-memory fakes of the ports, for tests (unit and `tests/`). Not used by
//! the binary.

use std::sync::{
    Mutex, MutexGuard, PoisonError,
    atomic::{AtomicU8, Ordering},
};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

use crate::{
    asp::AspAction,
    domain::{PoolField, TxHash},
    ports::{
        AnchorClient, AnchorDeposit, AnchorError, AnchorStatus, AspContract, AspKeys, AuditedTx,
        ChainError, Clock, Explorer, IncomingPayment, IntakeLedger, LedgerError, PaymentPage,
        PoolAuditor, PoolClient, PoolError, PoolTx, PreparedTx, PublicTx, StartDeposit, TxStatus,
    },
};

/// A clock that only moves when told to.
#[derive(Debug)]
pub struct FakeClock(Mutex<DateTime<Utc>>);

impl FakeClock {
    #[must_use]
    pub fn at(now: DateTime<Utc>) -> Self {
        Self(Mutex::new(now))
    }

    pub fn advance(&self, by: Duration) {
        let mut now = lock(&self.0);
        *now += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *lock(&self.0)
    }
}

/// How the fake chain answers the next call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainBehaviour {
    /// Every transaction lands successfully.
    Healthy,
    /// `prepare` fails as unavailable.
    Down,
    /// `submit` reports a timeout although the transaction lands.
    SubmitTimesOutButLands,
    /// `submit` succeeds but the transaction stays pending forever.
    NeverConfirms,
    /// `submit` succeeds but the network never sees the transaction.
    Drops,
    /// The network rejects the transaction.
    Rejects,
}

/// One write the fake chain applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedWrite {
    pub action: AspAction,
    pub key: PoolField,
}

#[derive(Debug, Default)]
struct ChainState {
    prepared: Vec<(TxHash, AspAction, PoolField)>,
    landed: Vec<TxHash>,
    pending: Vec<TxHash>,
    failed: Vec<TxHash>,
    applied: Vec<AppliedWrite>,
    submits: usize,
}

/// In-memory ASP contract with scriptable failures.
#[derive(Debug)]
pub struct FakeAspContract {
    behaviour: Mutex<ChainBehaviour>,
    state: Mutex<ChainState>,
    counter: AtomicU8,
    validity: Duration,
    clock_now: Mutex<DateTime<Utc>>,
}

impl Default for FakeAspContract {
    fn default() -> Self {
        Self {
            behaviour: Mutex::new(ChainBehaviour::Healthy),
            state: Mutex::new(ChainState::default()),
            counter: AtomicU8::new(0),
            validity: Duration::seconds(60),
            clock_now: Mutex::new(DateTime::UNIX_EPOCH),
        }
    }
}

impl FakeAspContract {
    pub fn set_behaviour(&self, behaviour: ChainBehaviour) {
        *lock(&self.behaviour) = behaviour;
    }

    /// Base time for `valid_until` of the next prepared transactions.
    pub fn set_now(&self, now: DateTime<Utc>) {
        *lock(&self.clock_now) = now;
    }

    /// Writes applied on-chain, in order.
    #[must_use]
    pub fn applied(&self) -> Vec<AppliedWrite> {
        lock(&self.state).applied.clone()
    }

    /// Number of `submit` calls (to prove retries do not resend).
    #[must_use]
    pub fn submits(&self) -> usize {
        lock(&self.state).submits
    }

    /// Makes a stuck pending transaction land now.
    pub fn confirm_pending(&self) {
        let mut state = lock(&self.state);
        let pending = std::mem::take(&mut state.pending);
        for hash in pending {
            Self::land(&mut state, &hash);
        }
    }

    fn land(state: &mut ChainState, hash: &TxHash) {
        if let Some((_, action, key)) = state.prepared.iter().find(|(h, ..)| h == hash).cloned() {
            state.applied.push(AppliedWrite { action, key });
        }
        state.landed.push(hash.clone());
    }
}

#[async_trait]
impl AspContract for FakeAspContract {
    async fn prepare(&self, action: AspAction, keys: &AspKeys) -> Result<PreparedTx, ChainError> {
        if *lock(&self.behaviour) == ChainBehaviour::Down {
            return Err(ChainError::Unavailable("fake chain is down".into()));
        }
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let hash = TxHash::from_bytes([n; 32]);
        let key = match action {
            AspAction::Allow => keys.asp_leaf.clone(),
            AspAction::Block | AspAction::Unblock => keys.note_public_key.clone(),
        };
        lock(&self.state).prepared.push((hash.clone(), action, key));
        Ok(PreparedTx {
            hash,
            envelope_xdr: format!("fake-envelope-{n}"),
            valid_until: *lock(&self.clock_now) + self.validity,
        })
    }

    async fn submit(&self, tx: &PreparedTx) -> Result<(), ChainError> {
        let behaviour = *lock(&self.behaviour);
        let mut state = lock(&self.state);
        state.submits += 1;
        match behaviour {
            ChainBehaviour::Healthy => {
                Self::land(&mut state, &tx.hash);
                Ok(())
            }
            ChainBehaviour::SubmitTimesOutButLands => {
                Self::land(&mut state, &tx.hash);
                Err(ChainError::Unavailable("submit timed out".into()))
            }
            ChainBehaviour::NeverConfirms => {
                state.pending.push(tx.hash.clone());
                Ok(())
            }
            ChainBehaviour::Drops => Ok(()),
            ChainBehaviour::Rejects => {
                state.failed.push(tx.hash.clone());
                Err(ChainError::Rejected("tx_bad_auth".into()))
            }
            ChainBehaviour::Down => Err(ChainError::Unavailable("fake chain is down".into())),
        }
    }

    async fn status(&self, hash: &TxHash) -> Result<TxStatus, ChainError> {
        let state = lock(&self.state);
        Ok(if state.landed.contains(hash) {
            TxStatus::Success
        } else if state.pending.contains(hash) {
            TxStatus::Pending
        } else if state.failed.contains(hash) {
            TxStatus::Failed("tx_bad_auth".into())
        } else {
            TxStatus::NotFound
        })
    }
}

/// How the fake anchor answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorBehaviour {
    Healthy,
    Down,
    Refuses,
}

/// In-memory SEP-24 anchor that records every request.
#[derive(Debug)]
pub struct FakeAnchor {
    behaviour: Mutex<AnchorBehaviour>,
    requests: Mutex<Vec<StartDeposit>>,
    statuses: Mutex<std::collections::HashMap<String, AnchorStatus>>,
    status_queries: Mutex<Vec<(i64, String)>>,
}

impl Default for FakeAnchor {
    fn default() -> Self {
        Self {
            behaviour: Mutex::new(AnchorBehaviour::Healthy),
            requests: Mutex::new(Vec::new()),
            statuses: Mutex::new(std::collections::HashMap::new()),
            status_queries: Mutex::new(Vec::new()),
        }
    }
}

impl FakeAnchor {
    pub const HOME_DOMAIN: &'static str = "anchor.example";

    pub fn set_behaviour(&self, behaviour: AnchorBehaviour) {
        *lock(&self.behaviour) = behaviour;
    }

    #[must_use]
    pub fn requests(&self) -> Vec<StartDeposit> {
        lock(&self.requests).clone()
    }

    /// Sets the status the anchor reports for a deposit (default: pending).
    pub fn set_status(&self, anchor_tx_id: &str, status: AnchorStatus) {
        lock(&self.statuses).insert(anchor_tx_id.to_owned(), status);
    }

    /// `(customer_memo, anchor_tx_id)` of every status query.
    #[must_use]
    pub fn status_queries(&self) -> Vec<(i64, String)> {
        lock(&self.status_queries).clone()
    }
}

#[async_trait]
impl AnchorClient for FakeAnchor {
    fn home_domain(&self) -> &str {
        Self::HOME_DOMAIN
    }

    async fn start_deposit(&self, request: &StartDeposit) -> Result<AnchorDeposit, AnchorError> {
        lock(&self.requests).push(request.clone());
        match *lock(&self.behaviour) {
            AnchorBehaviour::Healthy => Ok(AnchorDeposit {
                id: format!("anchor-tx-{}", request.deposit_memo),
                interactive_url: format!(
                    "https://{}/sep24/interactive?token=t{}",
                    Self::HOME_DOMAIN,
                    request.deposit_memo
                ),
            }),
            AnchorBehaviour::Down => Err(AnchorError::Unavailable("connection refused".into())),
            AnchorBehaviour::Refuses => Err(AnchorError::Refused("asset not supported".into())),
        }
    }

    async fn deposit_status(
        &self,
        customer_memo: i64,
        anchor_tx_id: &str,
    ) -> Result<AnchorStatus, AnchorError> {
        lock(&self.status_queries).push((customer_memo, anchor_tx_id.to_owned()));
        if *lock(&self.behaviour) == AnchorBehaviour::Down {
            return Err(AnchorError::Unavailable("connection refused".into()));
        }
        Ok(lock(&self.statuses)
            .get(anchor_tx_id)
            .cloned()
            .unwrap_or(AnchorStatus::Pending))
    }
}

/// In-memory Horizon: a list of the intake's payments, paged by cursor.
#[derive(Debug, Default)]
pub struct FakeLedger {
    payments: Mutex<Vec<IncomingPayment>>,
    down: Mutex<bool>,
}

impl FakeLedger {
    pub fn push(&self, payment: IncomingPayment) {
        lock(&self.payments).push(payment);
    }

    pub fn set_down(&self, down: bool) {
        *lock(&self.down) = down;
    }
}

#[async_trait]
impl IntakeLedger for FakeLedger {
    async fn payments_after(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaymentPage, LedgerError> {
        if *lock(&self.down) {
            return Err(LedgerError("horizon is down".into()));
        }
        // The cursor is the index of the next payment, as a string.
        let start: usize = cursor.and_then(|c| c.parse().ok()).unwrap_or(0);
        let all = lock(&self.payments);
        let page: Vec<IncomingPayment> = all
            .iter()
            .skip(start)
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .cloned()
            .collect();
        let next_cursor = (!page.is_empty()).then(|| (start + page.len()).to_string());
        Ok(PaymentPage {
            payments: page,
            next_cursor,
        })
    }
}

/// How the fake pool chain answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolBehaviour {
    /// Transactions land when their sequence is the next one.
    Healthy,
    /// `submit` succeeds but the network never sees the transaction.
    Drops,
    /// `submit` times out although the transaction lands.
    SubmitTimesOutButLands,
    /// The transaction lands but fails (sequence consumed, nothing applied).
    FailsOnChain,
    /// `submit` succeeds; the transaction stays pending.
    Pending,
    /// `submit` is refused (e.g. stale proof); nothing lands.
    Refuses,
}

/// One pool write the fake chain applied successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolWrite {
    Deposit {
        amount: i64,
    },
    Transfer {
        amount: i64,
        note_public_key: PoolField,
    },
}

#[derive(Debug, Clone)]
struct FakePoolTx {
    write: PoolWrite,
    sequence: i64,
    status: TxStatus,
}

#[derive(Debug)]
struct PoolState {
    behaviour: PoolBehaviour,
    prepare_fails: Option<bool>,
    intake_sequence: i64,
    txs: std::collections::HashMap<String, FakePoolTx>,
    applied: Vec<PoolWrite>,
    prepares: usize,
    submits: usize,
    counter: u8,
    /// `status` answers `NotFound` this many more times for landed
    /// transactions (RPC indexing lag).
    status_lag: usize,
}

/// In-memory pool with the real sequence rule: a transaction lands only if
/// its sequence is exactly the intake's next one.
#[derive(Debug)]
pub struct FakePool(Mutex<PoolState>);

impl Default for FakePool {
    fn default() -> Self {
        Self(Mutex::new(PoolState {
            behaviour: PoolBehaviour::Healthy,
            prepare_fails: None,
            intake_sequence: 100,
            txs: std::collections::HashMap::new(),
            applied: Vec::new(),
            prepares: 0,
            submits: 0,
            counter: 0,
            status_lag: 0,
        }))
    }
}

impl FakePool {
    pub fn set_behaviour(&self, behaviour: PoolBehaviour) {
        lock(&self.0).behaviour = behaviour;
    }

    /// Makes `prepare_*` fail; `Some(retryable)` or `None` to recover.
    pub fn set_prepare_fails(&self, retryable: Option<bool>) {
        lock(&self.0).prepare_fails = retryable;
    }

    /// Landed transactions look `NotFound` for the next `calls` status
    /// queries, while the account's sequence already moved.
    pub fn set_status_lag(&self, calls: usize) {
        lock(&self.0).status_lag = calls;
    }

    /// Another transaction from the intake used up the next sequence.
    pub fn bump_sequence(&self) {
        lock(&self.0).intake_sequence += 1;
    }

    /// Lands every pending transaction.
    pub fn confirm_pending(&self) {
        let mut state = lock(&self.0);
        let pending: Vec<String> = state
            .txs
            .iter()
            .filter(|(_, tx)| tx.status == TxStatus::Pending)
            .map(|(hash, _)| hash.clone())
            .collect();
        for hash in pending {
            Self::land(&mut state, &hash, true);
        }
    }

    #[must_use]
    pub fn applied(&self) -> Vec<PoolWrite> {
        lock(&self.0).applied.clone()
    }

    #[must_use]
    pub fn prepares(&self) -> usize {
        lock(&self.0).prepares
    }

    #[must_use]
    pub fn submits(&self) -> usize {
        lock(&self.0).submits
    }

    fn prepare(&self, write: PoolWrite) -> Result<PoolTx, PoolError> {
        let mut state = lock(&self.0);
        if let Some(retryable) = state.prepare_fails {
            return Err(PoolError {
                code: "chain".into(),
                message: "prover unavailable".into(),
                retryable,
            });
        }
        state.prepares += 1;
        state.counter = state.counter.wrapping_add(1);
        let hash = TxHash::from_bytes([state.counter; 32]);
        // Like the SDK: the next on-chain sequence, no local reservation.
        let sequence = state.intake_sequence + 1;
        state.txs.insert(
            hash.as_str().to_owned(),
            FakePoolTx {
                write,
                sequence,
                status: TxStatus::NotFound,
            },
        );
        Ok(PoolTx {
            envelope_xdr: hash.as_str().to_owned(),
            hash,
            sequence,
        })
    }

    /// Includes a transaction if its sequence is next; `success` applies it.
    fn land(state: &mut PoolState, hash: &str, success: bool) -> bool {
        let next = state.intake_sequence + 1;
        let Some(tx) = state.txs.get_mut(hash) else {
            return false;
        };
        if tx.sequence != next || matches!(tx.status, TxStatus::Success | TxStatus::Failed(_)) {
            return false;
        }
        state.intake_sequence = next;
        if success {
            tx.status = TxStatus::Success;
            state.applied.push(tx.write.clone());
        } else {
            tx.status = TxStatus::Failed("tx_failed".into());
        }
        true
    }
}

#[async_trait]
impl PoolClient for FakePool {
    async fn prepare_deposit(&self, amount: i64) -> Result<PoolTx, PoolError> {
        self.prepare(PoolWrite::Deposit { amount })
    }

    async fn prepare_transfer(
        &self,
        amount: i64,
        note_public_key: &PoolField,
        _encryption_public_key: &PoolField,
    ) -> Result<PoolTx, PoolError> {
        self.prepare(PoolWrite::Transfer {
            amount,
            note_public_key: note_public_key.clone(),
        })
    }

    async fn submit(&self, envelope_xdr: &str) -> Result<(), PoolError> {
        let mut state = lock(&self.0);
        state.submits += 1;
        let refused = |message: &str| PoolError {
            code: "sdk".into(),
            message: message.into(),
            retryable: false,
        };
        match state.behaviour {
            PoolBehaviour::Healthy => {
                let already = state
                    .txs
                    .get(envelope_xdr)
                    .is_some_and(|tx| tx.status == TxStatus::Success);
                if already || Self::land(&mut state, envelope_xdr, true) {
                    Ok(())
                } else {
                    Err(refused("tx_bad_seq"))
                }
            }
            PoolBehaviour::SubmitTimesOutButLands => {
                Self::land(&mut state, envelope_xdr, true);
                Err(PoolError {
                    code: "cli_timeout".into(),
                    message: "timed out".into(),
                    retryable: true,
                })
            }
            PoolBehaviour::FailsOnChain => {
                Self::land(&mut state, envelope_xdr, false);
                Ok(())
            }
            PoolBehaviour::Pending => {
                if let Some(tx) = state.txs.get_mut(envelope_xdr) {
                    tx.status = TxStatus::Pending;
                }
                Ok(())
            }
            PoolBehaviour::Drops => Ok(()),
            PoolBehaviour::Refuses => Err(refused("simulation failed: stale ASP root")),
        }
    }

    async fn status(&self, hash: &TxHash) -> Result<TxStatus, PoolError> {
        let mut state = lock(&self.0);
        let status = state
            .txs
            .get(hash.as_str())
            .map_or(TxStatus::NotFound, |tx| tx.status.clone());
        if status == TxStatus::Success && state.status_lag > 0 {
            state.status_lag -= 1;
            return Ok(TxStatus::NotFound);
        }
        Ok(status)
    }

    async fn intake_sequence(&self) -> Result<i64, PoolError> {
        Ok(lock(&self.0).intake_sequence)
    }
}

/// The auditor's decrypted pool history, newest first; can be made to fail.
#[derive(Debug, Default)]
pub struct FakeAuditor {
    txs: Mutex<Vec<AuditedTx>>,
    fails: Mutex<bool>,
    limits: Mutex<Vec<usize>>,
}

impl FakeAuditor {
    #[must_use]
    pub fn with(txs: Vec<AuditedTx>) -> Self {
        Self {
            txs: Mutex::new(txs),
            ..Self::default()
        }
    }

    pub fn set_fails(&self, fails: bool) {
        *lock(&self.fails) = fails;
    }

    /// The `limit` of every call so far.
    #[must_use]
    pub fn limits(&self) -> Vec<usize> {
        lock(&self.limits).clone()
    }
}

#[async_trait]
impl PoolAuditor for FakeAuditor {
    async fn audit(&self, limit: usize) -> Result<Vec<AuditedTx>, PoolError> {
        lock(&self.limits).push(limit);
        if *lock(&self.fails) {
            return Err(PoolError {
                code: "cli_timeout".into(),
                message: "pool-cli timed out".into(),
                retryable: true,
            });
        }
        Ok(lock(&self.txs).iter().take(limit).cloned().collect())
    }
}

/// Public transactions by hash and pool transactions by ledger.
#[derive(Debug, Default)]
pub struct FakeExplorer {
    txs: Mutex<Vec<PublicTx>>,
    ledgers: Mutex<Vec<(u32, Vec<TxHash>)>>,
    fails: Mutex<bool>,
    ledger_queries: Mutex<Vec<u32>>,
}

impl FakeExplorer {
    pub fn add_tx(&self, tx: PublicTx) {
        lock(&self.txs).push(tx);
    }

    pub fn set_ledger(&self, ledger: u32, hashes: Vec<TxHash>) {
        lock(&self.ledgers).push((ledger, hashes));
    }

    pub fn set_fails(&self, fails: bool) {
        *lock(&self.fails) = fails;
    }

    #[must_use]
    pub fn ledger_queries(&self) -> Vec<u32> {
        lock(&self.ledger_queries).clone()
    }

    fn check(&self) -> Result<(), LedgerError> {
        if *lock(&self.fails) {
            Err(LedgerError(
                "Horizon answered 503 Service Unavailable".into(),
            ))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl Explorer for FakeExplorer {
    async fn transaction(&self, hash: &TxHash) -> Result<Option<PublicTx>, LedgerError> {
        self.check()?;
        Ok(lock(&self.txs).iter().find(|tx| &tx.hash == hash).cloned())
    }

    async fn pool_transactions_in_ledger(&self, ledger: u32) -> Result<Vec<TxHash>, LedgerError> {
        lock(&self.ledger_queries).push(ledger);
        self.check()?;
        Ok(lock(&self.ledgers)
            .iter()
            .find(|(l, _)| *l == ledger)
            .map(|(_, hashes)| hashes.clone())
            .unwrap_or_default())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
