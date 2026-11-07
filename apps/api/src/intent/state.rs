//! The ramp intent state machine (AGENTS.md §6.4). Pure: the single source
//! of truth for which status changes are legal. Every write goes through
//! [`check`]; an invalid transition is an error, never a no-op.
//!
//! ```text
//! created ──► awaiting_anchor ──► funds_received ──► shielding ──► transferring ──► completed
//!    │               │                  │                │               │
//!    ├─► rejected    ├─► expired        └─► error        └─► error       └─► error
//!    └─► error       ├─► error
//!                    └─► rejected
//! ```
//!
//! - `created → error`: the anchor could not start the deposit.
//! - `created | awaiting_anchor → rejected`: the user's key was frozen
//!   before any money reached Shield. Once funds arrived they are in Shield's
//!   custody and the intent can only end in `completed` or `error` (manual
//!   review), never silently dropped.

use std::{fmt, str::FromStr};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentStatus {
    Created,
    AwaitingAnchor,
    FundsReceived,
    Shielding,
    Transferring,
    Completed,
    Rejected,
    Expired,
    Error,
}

impl IntentStatus {
    pub const ALL: [Self; 9] = [
        Self::Created,
        Self::AwaitingAnchor,
        Self::FundsReceived,
        Self::Shielding,
        Self::Transferring,
        Self::Completed,
        Self::Rejected,
        Self::Expired,
        Self::Error,
    ];

    /// No transition leaves a final status.
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Rejected | Self::Expired | Self::Error
        )
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::AwaitingAnchor => "awaiting_anchor",
            Self::FundsReceived => "funds_received",
            Self::Shielding => "shielding",
            Self::Transferring => "transferring",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for IntentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for IntentStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == s)
            .ok_or_else(|| format!("unknown intent status `{s}`"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid intent transition {from} → {to}")]
pub struct InvalidTransition {
    pub from: IntentStatus,
    pub to: IntentStatus,
}

/// Whether `from → to` is a legal transition.
pub fn check(from: IntentStatus, to: IntentStatus) -> Result<(), InvalidTransition> {
    use IntentStatus::{
        AwaitingAnchor, Completed, Created, Error, Expired, FundsReceived, Rejected, Shielding,
        Transferring,
    };
    let legal = matches!(
        (from, to),
        (Created, AwaitingAnchor | Rejected | Error)
            | (AwaitingAnchor, FundsReceived | Expired | Error | Rejected)
            | (FundsReceived, Shielding | Error)
            | (Shielding, Transferring | Error)
            | (Transferring, Completed | Error)
    );
    if legal {
        Ok(())
    } else {
        Err(InvalidTransition { from, to })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{IntentStatus::*, *};

    const LEGAL: [(IntentStatus, IntentStatus); 14] = [
        (Created, AwaitingAnchor),
        (Created, Rejected),
        (Created, Error),
        (AwaitingAnchor, FundsReceived),
        (AwaitingAnchor, Expired),
        (AwaitingAnchor, Error),
        (AwaitingAnchor, Rejected),
        (FundsReceived, Shielding),
        (FundsReceived, Error),
        (Shielding, Transferring),
        (Shielding, Error),
        (Transferring, Completed),
        (Transferring, Error),
        (Created, AwaitingAnchor), // repeated on purpose: the set dedupes
    ];

    #[test]
    fn transition_table_is_exactly_the_documented_one() {
        let legal: HashSet<_> = LEGAL.into_iter().collect();
        assert_eq!(legal.len(), 13);
        for from in IntentStatus::ALL {
            for to in IntentStatus::ALL {
                assert_eq!(
                    check(from, to).is_ok(),
                    legal.contains(&(from, to)),
                    "{from} → {to}"
                );
            }
        }
    }

    #[test]
    fn rejects_transition_from_completed_to_shielding() {
        assert_eq!(
            check(Completed, Shielding),
            Err(InvalidTransition {
                from: Completed,
                to: Shielding
            })
        );
    }

    #[test]
    fn final_states_have_no_way_out() {
        for from in IntentStatus::ALL.into_iter().filter(|s| s.is_final()) {
            for to in IntentStatus::ALL {
                assert!(check(from, to).is_err(), "{from} → {to}");
            }
        }
    }

    #[test]
    fn no_state_transitions_to_itself() {
        for status in IntentStatus::ALL {
            assert!(check(status, status).is_err(), "{status}");
        }
    }

    #[test]
    fn funds_in_custody_cannot_be_rejected_or_expired() {
        for from in [FundsReceived, Shielding, Transferring] {
            assert!(check(from, Rejected).is_err(), "{from}");
            assert!(check(from, Expired).is_err(), "{from}");
        }
    }

    #[test]
    fn the_happy_path_is_reachable_in_order() {
        let path = [
            Created,
            AwaitingAnchor,
            FundsReceived,
            Shielding,
            Transferring,
            Completed,
        ];
        for pair in path.windows(2) {
            assert!(check(pair[0], pair[1]).is_ok(), "{} → {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn every_non_final_state_can_reach_a_final_state() {
        // Breadth-first search over the legal table.
        for start in IntentStatus::ALL.into_iter().filter(|s| !s.is_final()) {
            let mut seen = HashSet::from([start]);
            let mut frontier = vec![start];
            while let Some(status) = frontier.pop() {
                for next in IntentStatus::ALL {
                    if check(status, next).is_ok() && seen.insert(next) {
                        frontier.push(next);
                    }
                }
            }
            assert!(seen.iter().any(|s| s.is_final()), "{start} is a dead end");
        }
    }

    #[test]
    fn text_round_trips() {
        for status in IntentStatus::ALL {
            assert_eq!(status.as_str().parse::<IntentStatus>(), Ok(status));
        }
        assert!("done".parse::<IntentStatus>().is_err());
        assert_eq!(
            serde_json::to_string(&AwaitingAnchor).ok().as_deref(),
            Some("\"awaiting_anchor\"")
        );
    }
}
