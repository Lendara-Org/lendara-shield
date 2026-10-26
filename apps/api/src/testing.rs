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
        AnchorClient, AnchorDeposit, AnchorError, AnchorStatus, AspContract, AspKeys, ChainError,
        Clock, IncomingPayment, IntakeLedger, LedgerError, PaymentPage, PreparedTx, StartDeposit,
        TxStatus,
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
