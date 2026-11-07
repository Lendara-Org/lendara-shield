//! Background loops of the ramp, stopped cleanly on shutdown:
//! - the ramp loop reads the intake's payments, then follows deposits at the
//!   anchor (payments first, so a paid intent is never expired);
//! - the shielding loop moves funded intents into the pool, one at a time
//!   (the intake has a single sequence number and SDK wallet).
//!
//! They run side by side so a slow proof (`pool-cli`, up to its timeout)
//! never delays reading payments or anchor statuses. They share only the
//! database, where every status change is guarded by the expected status.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::intent::{anchor_sync::AnchorSync, shielding::ShieldingWorker, watcher::IntakeWatcher};

/// Time between two rounds. Testnet closes a ledger every ~5 s.
pub const TICK: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct Background {
    pub watcher: IntakeWatcher,
    pub anchor_sync: AnchorSync,
    /// `None` until the pool CLI is configured.
    pub shielding: Option<ShieldingWorker>,
}

impl Background {
    /// Runs both loops until `shutdown` is cancelled. A failing step is
    /// logged and retried on the next round; it never stops a loop.
    pub async fn run(self, tick: Duration, shutdown: CancellationToken) {
        tokio::join!(
            every(tick, &shutdown, || self.ramp_round()),
            every(tick, &shutdown, || self.shielding_round()),
        );
        tracing::info!("background loops stopped");
    }

    /// One round of everything, in order (for tests and one-off runs).
    pub async fn round(&self) {
        self.ramp_round().await;
        self.shielding_round().await;
    }

    /// Payments first (a paid intent is never expired), then anchor statuses.
    pub async fn ramp_round(&self) {
        match self.watcher.poll().await {
            Ok(report) if report.recorded > 0 => {
                tracing::info!(
                    recorded = report.recorded,
                    matched = report.matched,
                    "intake payments processed"
                );
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(error = %err, "intake watcher failed; retrying next round"),
        }
        if let Err(err) = self.anchor_sync.check_due().await {
            tracing::warn!(error = %err, "anchor sync failed; retrying next round");
        }
    }

    /// Advances funded intents (no-op until the pool CLI is configured).
    pub async fn shielding_round(&self) {
        if let Some(shielding) = &self.shielding
            && let Err(err) = shielding.process_due().await
        {
            tracing::warn!(error = %err, "shielding worker failed; retrying next round");
        }
    }
}

/// Runs `round` every `tick` until `shutdown` is cancelled. A round in
/// progress is dropped on shutdown: every step persists its state before
/// any network call, so it resumes from the database on the next start.
async fn every<F, Fut>(tick: Duration, shutdown: &CancellationToken, round: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut interval = tokio::time::interval(tick);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            () = shutdown.cancelled() => break,
            _ = interval.tick() => {
                tokio::select! {
                    () = shutdown.cancelled() => break,
                    () = round() => {}
                }
            }
        }
    }
}
