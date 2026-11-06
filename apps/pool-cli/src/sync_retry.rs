//! In-process retry of a proof preparation that the SDK refused with
//! `SyncRequired`.
//!
//! The SDK's membership precondition answers `SyncRequired` whenever the
//! chain moved past the wallet's index between its sync and its state read,
//! which on testnet (a ledger every ~5 s) happens often even when nothing
//! changed in the allow-list. The SDK's own executor (`PrivatePool::execute`)
//! absorbs this by syncing and retrying in-process; `pool-cli` drives the
//! lower-level `prove_next` (so the API can persist each hash before
//! submitting) and used to hand every refusal back to the worker, which then
//! paid a whole new process, prover setup and sync per retry (30–60 s).
//!
//! Retrying here is safe: nothing has been signed or sent yet.

use std::time::Duration;

use stellar_private_payments::{Error as SdkError, types::AspMembershipSync};

/// Attempts within one run before handing `SyncRequired` back to the worker:
/// each takes a sync (a few seconds) plus [`PAUSE`], well under the API's
/// 180 s budget for a `pool-cli` run.
pub const MAX_ATTEMPTS: u32 = 10;
/// Pause between attempts: a ledger closes about every 5 s on testnet, so a
/// short wait gives the next sync and state read a chance to agree.
pub const PAUSE: Duration = Duration::from_secs(1);

/// Whether `err` is the SDK asking for a sync (not a real failure).
fn is_sync_required(err: &SdkError) -> bool {
    matches!(
        err,
        SdkError::MembershipSync(AspMembershipSync::SyncRequired(_))
    )
}

/// How many more times a preparation refused with `SyncRequired` may run in
/// this process. Anything else, or an exhausted budget, goes back to the
/// worker unchanged (so it still waits for the wallet without spending an
/// attempt).
#[derive(Debug)]
pub struct SyncRetry {
    remaining: u32,
    pause: Duration,
}

impl SyncRetry {
    /// `attempts` runs in total, `pause` between them.
    #[must_use]
    pub fn new(attempts: u32, pause: Duration) -> Self {
        Self {
            remaining: attempts.saturating_sub(1),
            pause,
        }
    }

    /// After a failed run: waits and returns `true` if it should run again.
    pub async fn retry_after(&mut self, err: &SdkError) -> bool {
        if !is_sync_required(err) || self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        wait(self.pause).await;
        true
    }
}

/// Sleeps without blocking the runtime (tokio's `time` feature is off).
async fn wait(pause: Duration) {
    if pause.is_zero() {
        return;
    }
    // A failed join only shortens the pause.
    let _ = tokio::task::spawn_blocking(move || std::thread::sleep(pause)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sync_required() -> SdkError {
        SdkError::MembershipSync(AspMembershipSync::SyncRequired(Some(1)))
    }

    /// Runs a fake preparation that answers `results` in order, like the
    /// loop in `pool::prepare`; returns the outcome and how many runs it took.
    async fn drive(
        attempts: u32,
        mut results: Vec<Result<u32, SdkError>>,
    ) -> (Result<u32, SdkError>, usize) {
        results.reverse();
        let mut retry = SyncRetry::new(attempts, Duration::ZERO);
        let mut runs = 0;
        loop {
            runs += 1;
            match results.pop().unwrap_or_else(|| Err(sync_required())) {
                Ok(v) => return (Ok(v), runs),
                Err(e) if retry.retry_after(&e).await => {}
                Err(e) => return (Err(e), runs),
            }
        }
    }

    #[tokio::test]
    async fn retries_a_moving_tip_until_the_preparation_succeeds() {
        let (result, runs) =
            drive(5, vec![Err(sync_required()), Err(sync_required()), Ok(7)]).await;
        assert!(matches!(result, Ok(7)));
        assert_eq!(runs, 3);
    }

    #[tokio::test]
    async fn hands_sync_required_back_after_the_last_attempt() {
        let (result, runs) = drive(4, vec![]).await;
        assert!(matches!(
            result,
            Err(SdkError::MembershipSync(AspMembershipSync::SyncRequired(
                Some(1)
            )))
        ));
        assert_eq!(runs, 4);
    }

    #[tokio::test]
    async fn never_retries_a_real_failure() {
        let (result, runs) = drive(
            5,
            vec![Err(SdkError::MembershipSync(
                AspMembershipSync::RegisterAtASP,
            ))],
        )
        .await;
        assert!(matches!(
            result,
            Err(SdkError::MembershipSync(AspMembershipSync::RegisterAtASP))
        ));
        assert_eq!(runs, 1);
    }

    #[tokio::test]
    async fn a_single_attempt_budget_behaves_like_no_retry() {
        let (_, runs) = drive(1, vec![]).await;
        assert_eq!(runs, 1);
    }
}
