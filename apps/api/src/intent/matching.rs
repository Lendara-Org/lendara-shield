//! Does an incoming payment fund an intent? Pure.
//!
//! A payment funds an intent only if it is in the pool's asset (code **and**
//! issuer: anyone can issue a token called USDC), carries the intent's `id`
//! memo, and the intent is still waiting for the anchor. Anything else is
//! recorded as unmatched for manual review, never silently dropped: it is
//! money in Shield's custody.

use super::state::IntentStatus;
use crate::ports::IncomingPayment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unmatched {
    /// Wrong asset code or issuer.
    WrongAsset,
    /// The transaction has no `id` memo.
    NoMemo,
    /// No intent has this memo.
    UnknownMemo,
    /// The intent exists but is past `awaiting_anchor` (already funded,
    /// expired, ...): a second or late payment.
    NotAwaiting(IntentStatus),
}

impl Unmatched {
    /// Stored in `intake_payments.unmatched_reason`.
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::WrongAsset => "wrong asset".to_owned(),
            Self::NoMemo => "no id memo".to_owned(),
            Self::UnknownMemo => "no intent with this memo".to_owned(),
            Self::NotAwaiting(status) => format!("intent is {status}"),
        }
    }
}

/// The asset the pool holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedAsset {
    pub code: String,
    pub issuer: String,
}

/// Classifies `payment` given the status of the intent its memo points to
/// (`None` if there is no such intent).
pub fn classify(
    payment: &IncomingPayment,
    asset: &ExpectedAsset,
    intent: Option<IntentStatus>,
) -> Result<(), Unmatched> {
    if payment.asset_code != asset.code || payment.asset_issuer != asset.issuer {
        return Err(Unmatched::WrongAsset);
    }
    if payment.memo.is_none() {
        return Err(Unmatched::NoMemo);
    }
    match intent {
        None => Err(Unmatched::UnknownMemo),
        Some(IntentStatus::AwaitingAnchor) => Ok(()),
        Some(status) => Err(Unmatched::NotAwaiting(status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AccountId, DecimalAmount, TxHash};

    const ISSUER: &str = "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5";

    fn asset() -> ExpectedAsset {
        ExpectedAsset {
            code: "USDC".into(),
            issuer: ISSUER.into(),
        }
    }

    fn payment(
        code: &str,
        issuer: &str,
        memo: Option<u64>,
    ) -> Result<IncomingPayment, Box<dyn std::error::Error>> {
        Ok(IncomingPayment {
            operation_id: "1".into(),
            tx_hash: TxHash::from_bytes([1; 32]),
            from: AccountId::parse("GAO7YFNVZAHK6RSKV2LNPIS3WDGCHLBRLIFQETTPE36TLFFAGENNMNT5")?,
            asset_code: code.into(),
            asset_issuer: issuer.into(),
            amount: DecimalAmount::parse("9.3039215")?,
            memo,
        })
    }

    #[test]
    fn matches_the_pool_asset_with_the_intent_memo() -> Result<(), Box<dyn std::error::Error>> {
        let p = payment("USDC", ISSUER, Some(2))?;
        assert_eq!(
            classify(&p, &asset(), Some(IntentStatus::AwaitingAnchor)),
            Ok(())
        );
        Ok(())
    }

    #[test]
    fn rejects_a_fake_usdc_from_another_issuer() -> Result<(), Box<dyn std::error::Error>> {
        let fake = payment(
            "USDC",
            "GAO7YFNVZAHK6RSKV2LNPIS3WDGCHLBRLIFQETTPE36TLFFAGENNMNT5",
            Some(2),
        )?;
        assert_eq!(
            classify(&fake, &asset(), Some(IntentStatus::AwaitingAnchor)),
            Err(Unmatched::WrongAsset)
        );
        let other = payment("EURC", ISSUER, Some(2))?;
        assert_eq!(
            classify(&other, &asset(), Some(IntentStatus::AwaitingAnchor)),
            Err(Unmatched::WrongAsset)
        );
        Ok(())
    }

    #[test]
    fn needs_a_memo_and_a_known_intent() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            classify(&payment("USDC", ISSUER, None)?, &asset(), None),
            Err(Unmatched::NoMemo)
        );
        assert_eq!(
            classify(&payment("USDC", ISSUER, Some(9))?, &asset(), None),
            Err(Unmatched::UnknownMemo)
        );
        Ok(())
    }

    #[test]
    fn a_second_or_late_payment_is_not_matched() -> Result<(), Box<dyn std::error::Error>> {
        let p = payment("USDC", ISSUER, Some(2))?;
        for status in IntentStatus::ALL
            .into_iter()
            .filter(|s| *s != IntentStatus::AwaitingAnchor)
        {
            assert_eq!(
                classify(&p, &asset(), Some(status)),
                Err(Unmatched::NotAwaiting(status)),
                "{status}"
            );
        }
        assert_eq!(
            Unmatched::NotAwaiting(IntentStatus::Expired).reason(),
            "intent is expired"
        );
        Ok(())
    }
}
