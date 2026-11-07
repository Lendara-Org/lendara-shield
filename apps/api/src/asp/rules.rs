//! Pure rules of the simulated KYC and the ASP status. No I/O.
//!
//! The pool's allow-list is append-only (`insert_leaf` only), so a key is
//! frozen by adding it to the block-list and unfrozen by removing it from
//! there. `not_allowed` is final: a rejected KYC never touches the chain.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Simulated KYC outcome chosen at onboarding (sandbox only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KycScenario {
    /// KYC passes; the key joins the allow-list.
    #[default]
    Approved,
    /// KYC fails; the key never reaches the chain.
    Rejected,
    /// KYC passes, then the key is frozen: demonstrates the block-list.
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KycStatus {
    Approved,
    Rejected,
}

/// Where the user's key stands in the pool's ASP trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AspStatus {
    /// An on-chain update is needed or in flight.
    Pending,
    /// In the allow-list and not blocked: can transact.
    Allowed,
    /// KYC rejected; never inserted. Final.
    NotAllowed,
    /// In the block-list: frozen.
    Blocked,
}

/// One on-chain ASP update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AspAction {
    /// Insert the ASP leaf into the allow-list (membership tree).
    Allow,
    /// Insert the note public key into the block-list (non-membership tree).
    Block,
    /// Remove the note public key from the block-list.
    Unblock,
}

/// Why an ASP action is not possible from the current status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("cannot {action} a user whose ASP status is {status}")]
pub struct TransitionError {
    pub action: AspAction,
    pub status: AspStatus,
}

/// Outcome of the simulated KYC: the stored statuses and the on-chain
/// updates still to run, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KycOutcome {
    pub kyc_status: KycStatus,
    pub asp_status: AspStatus,
    pub actions: Vec<AspAction>,
}

/// Runs the simulated KYC.
#[must_use]
pub fn run_kyc(scenario: KycScenario) -> KycOutcome {
    match scenario {
        KycScenario::Approved => KycOutcome {
            kyc_status: KycStatus::Approved,
            asp_status: AspStatus::Pending,
            actions: vec![AspAction::Allow],
        },
        KycScenario::Blocked => KycOutcome {
            kyc_status: KycStatus::Approved,
            asp_status: AspStatus::Pending,
            actions: vec![AspAction::Allow, AspAction::Block],
        },
        KycScenario::Rejected => KycOutcome {
            kyc_status: KycStatus::Rejected,
            asp_status: AspStatus::NotAllowed,
            actions: Vec::new(),
        },
    }
}

/// Actions still owed to reach the scenario's target, given which actions
/// were already confirmed on-chain (resumes an interrupted onboarding).
#[must_use]
pub fn remaining_onboarding_actions(
    scenario: KycScenario,
    confirmed: &[AspAction],
) -> Vec<AspAction> {
    run_kyc(scenario)
        .actions
        .into_iter()
        .filter(|action| !confirmed.contains(action))
        .collect()
}

/// Checks that `action` is allowed from `status`. Pending is the only state
/// in which an update may already be in flight, so admin actions require a
/// settled state.
pub fn check_transition(status: AspStatus, action: AspAction) -> Result<(), TransitionError> {
    let allowed = matches!(
        (status, action),
        (AspStatus::Pending, AspAction::Allow | AspAction::Block)
            | (AspStatus::Allowed, AspAction::Block)
            | (AspStatus::Blocked, AspAction::Unblock)
    );
    if allowed {
        Ok(())
    } else {
        Err(TransitionError { action, status })
    }
}

/// Status once `action` is confirmed on-chain. `remaining` are the
/// onboarding actions still owed after this one: while any remain, the user
/// stays `pending` (e.g. `blocked` scenario between allow and block).
#[must_use]
pub fn status_after(action: AspAction, remaining: usize) -> AspStatus {
    if remaining > 0 {
        return AspStatus::Pending;
    }
    match action {
        AspAction::Allow | AspAction::Unblock => AspStatus::Allowed,
        AspAction::Block => AspStatus::Blocked,
    }
}

macro_rules! text_enum {
    ($ty:ty { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl $ty {
            /// Value stored in the database and sent in the API.
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $ty {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok(Self::$variant),)+
                    other => Err(format!("unknown {} `{other}`", stringify!($ty))),
                }
            }
        }
    };
}

text_enum!(KycScenario { Approved => "approved", Rejected => "rejected", Blocked => "blocked" });
text_enum!(KycStatus { Approved => "approved", Rejected => "rejected" });
text_enum!(AspStatus {
    Pending => "pending",
    Allowed => "allowed",
    NotAllowed => "not_allowed",
    Blocked => "blocked",
});
text_enum!(AspAction { Allow => "allow", Block => "block", Unblock => "unblock" });

#[cfg(test)]
mod tests {
    use super::*;

    const STATUSES: [AspStatus; 4] = [
        AspStatus::Pending,
        AspStatus::Allowed,
        AspStatus::NotAllowed,
        AspStatus::Blocked,
    ];
    const ACTIONS: [AspAction; 3] = [AspAction::Allow, AspAction::Block, AspAction::Unblock];

    #[test]
    fn approved_kyc_allows_the_key() {
        assert_eq!(
            run_kyc(KycScenario::Approved),
            KycOutcome {
                kyc_status: KycStatus::Approved,
                asp_status: AspStatus::Pending,
                actions: vec![AspAction::Allow],
            }
        );
    }

    #[test]
    fn rejected_kyc_never_touches_the_chain() {
        let outcome = run_kyc(KycScenario::Rejected);
        assert_eq!(outcome.kyc_status, KycStatus::Rejected);
        assert_eq!(outcome.asp_status, AspStatus::NotAllowed);
        assert!(outcome.actions.is_empty());
    }

    #[test]
    fn blocked_scenario_allows_then_blocks() {
        assert_eq!(
            run_kyc(KycScenario::Blocked).actions,
            vec![AspAction::Allow, AspAction::Block]
        );
    }

    #[test]
    fn resuming_skips_confirmed_actions() {
        use AspAction::{Allow, Block};
        assert_eq!(
            remaining_onboarding_actions(KycScenario::Blocked, &[]),
            vec![Allow, Block]
        );
        assert_eq!(
            remaining_onboarding_actions(KycScenario::Blocked, &[Allow]),
            vec![Block]
        );
        assert!(remaining_onboarding_actions(KycScenario::Blocked, &[Allow, Block]).is_empty());
        assert!(remaining_onboarding_actions(KycScenario::Rejected, &[]).is_empty());
    }

    #[test]
    fn transition_table_is_exhaustive() {
        use AspAction::{Allow, Block, Unblock};
        use AspStatus::{Allowed, Blocked, Pending};
        let valid = [
            (Pending, Allow),
            (Pending, Block),
            (Allowed, Block),
            (Blocked, Unblock),
        ];
        for status in STATUSES {
            for action in ACTIONS {
                let expected = valid.contains(&(status, action));
                assert_eq!(
                    check_transition(status, action).is_ok(),
                    expected,
                    "{action} from {status}"
                );
            }
        }
    }

    #[test]
    fn not_allowed_is_final() {
        for action in ACTIONS {
            assert_eq!(
                check_transition(AspStatus::NotAllowed, action),
                Err(TransitionError {
                    action,
                    status: AspStatus::NotAllowed
                })
            );
        }
    }

    #[test]
    fn status_after_confirmation() {
        assert_eq!(status_after(AspAction::Allow, 0), AspStatus::Allowed);
        assert_eq!(status_after(AspAction::Allow, 1), AspStatus::Pending);
        assert_eq!(status_after(AspAction::Block, 0), AspStatus::Blocked);
        assert_eq!(status_after(AspAction::Unblock, 0), AspStatus::Allowed);
    }

    #[test]
    fn text_round_trips() {
        for status in STATUSES {
            assert_eq!(status.as_str().parse::<AspStatus>(), Ok(status));
        }
        for action in ACTIONS {
            assert_eq!(action.as_str().parse::<AspAction>(), Ok(action));
        }
        for scenario in [
            KycScenario::Approved,
            KycScenario::Rejected,
            KycScenario::Blocked,
        ] {
            assert_eq!(scenario.as_str().parse::<KycScenario>(), Ok(scenario));
        }
        assert!("frozen".parse::<AspStatus>().is_err());
        assert_eq!("approved".parse::<KycStatus>(), Ok(KycStatus::Approved));
    }

    #[test]
    fn scenario_defaults_to_approved() {
        assert_eq!(KycScenario::default(), KycScenario::Approved);
    }
}
