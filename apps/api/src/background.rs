//! Background loop of the ramp: reads the intake's payments, shields funded
//! intents and follows deposits at the anchor. One task, sequential steps
//! (the intake has a single sequence number and SDK wallet), stopped cleanly
//! on shutdown.

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
    /// Runs rounds until `shutdown` is cancelled. A failing step is logged
    /// and retried on the next round; it never stops the loop.
    pub async fn run(self, tick: Duration, shutdown: CancellationToken) {
        let mut interval = tokio::time::interval(tick);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                _ = interval.tick() => self.round().await,
            }
        }
        tracing::info!("background loop stopped");
    }

    /// One round: payments first (a paid intent is never expired), then
    /// shielding, then anchor statuses.
    pub async fn round(&self) {
        match self.watcher.poll().await {
            Ok(report) if report.recorded > 0 => {
                tracing::info!(
                    recorded = report.recorded,
                    funded = report.funded,
                    "intake payments processed"
                );
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(error = %err, "intake watcher failed; retrying next round"),
        }
        if let Some(shielding) = &self.shielding
            && let Err(err) = shielding.process_due().await
        {
            tracing::warn!(error = %err, "shielding worker failed; retrying next round");
        }
        if let Err(err) = self.anchor_sync.check_due().await {
            tracing::warn!(error = %err, "anchor sync failed; retrying next round");
        }
    }
}
