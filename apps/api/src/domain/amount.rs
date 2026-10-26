//! Positive decimal amounts with Stellar precision (7 decimals), kept as
//! text end to end: never `f64` (CLAUDE.md §0.6).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Most decimals Stellar assets have.
pub const MAX_DECIMALS: usize = 7;
/// Most integer digits accepted (fits `numeric(20, 7)` and `i64` stroops).
const MAX_INTEGER_DIGITS: usize = 11;

/// A positive amount in canonical form: no leading zeros, no trailing
/// fractional zeros, no trailing dot (`100`, `0.5`, `12.3456789`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DecimalAmount(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a positive decimal amount with up to 7 decimals")]
pub struct AmountError(pub String);

impl DecimalAmount {
    pub fn parse(raw: &str) -> Result<Self, AmountError> {
        let err = || AmountError(raw.to_owned());
        let (int, frac) = raw.split_once('.').unwrap_or((raw, ""));
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if int.is_empty()
            || !digits(int)
            || !digits(frac)
            || (raw.contains('.') && frac.is_empty())
            || frac.len() > MAX_DECIMALS
        {
            return Err(err());
        }
        let int = int.trim_start_matches('0');
        let frac = frac.trim_end_matches('0');
        if int.len() > MAX_INTEGER_DIGITS || (int.is_empty() && frac.is_empty()) {
            return Err(err());
        }
        let int = if int.is_empty() { "0" } else { int };
        Ok(Self(if frac.is_empty() {
            int.to_owned()
        } else {
            format!("{int}.{frac}")
        }))
    }

    /// The amount in stroops (1e-7 units).
    #[must_use]
    pub fn to_stroops(&self) -> i64 {
        let (int, frac) = self.0.split_once('.').unwrap_or((&self.0, ""));
        let padded = format!("{frac:0<MAX_DECIMALS$}");
        // Validated in `parse`: at most 11 + 7 digits, which fits i64.
        format!("{int}{padded}").parse().unwrap_or(i64::MAX)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DecimalAmount {
    type Error = AmountError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<DecimalAmount> for String {
    fn from(value: DecimalAmount) -> Self {
        value.0
    }
}

impl fmt::Display for DecimalAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Formats a signed amount of stroops as a canonical decimal (`-0.5`, `0`,
/// `9.3039215`), as `SignedDecimalAmount` in the API contract.
#[must_use]
pub fn format_stroops(stroops: i128) -> String {
    let sign = if stroops < 0 { "-" } else { "" };
    let abs = stroops.unsigned_abs();
    let scale = 10u128.pow(7);
    let frac = format!("{:07}", abs % scale);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{sign}{}", abs / scale)
    } else {
        format!("{sign}{}.{frac}", abs / scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_signed_stroops() {
        for (stroops, text) in [
            (0, "0"),
            (93_039_215, "9.3039215"),
            (10_000_000, "1"),
            (-5_000_000, "-0.5"),
            (1, "0.0000001"),
            (-1, "-0.0000001"),
        ] {
            assert_eq!(format_stroops(stroops), text);
        }
    }

    #[test]
    fn formatting_round_trips_with_parse() -> Result<(), AmountError> {
        for raw in ["9.3039215", "0.5", "100", "0.0000001"] {
            let amount = DecimalAmount::parse(raw)?;
            assert_eq!(format_stroops(i128::from(amount.to_stroops())), raw);
        }
        Ok(())
    }

    #[test]
    fn canonicalises() -> Result<(), AmountError> {
        for (raw, canonical) in [
            ("100", "100"),
            ("100.00", "100"),
            ("0100.50", "100.5"),
            ("0.5", "0.5"),
            ("12.3456789", "12.3456789"),
            ("99999999999.9999999", "99999999999.9999999"),
        ] {
            assert_eq!(DecimalAmount::parse(raw)?.as_str(), canonical, "{raw}");
        }
        Ok(())
    }

    #[test]
    fn rejects_invalid_amounts() {
        for raw in [
            "",
            "0",
            "0.0",
            "-1",
            "+1",
            "1e3",
            "1,5",
            ".5",
            "5.",
            "1.23456789",
            " 1",
            "abc",
            "100000000000",
            "NaN",
        ] {
            assert!(DecimalAmount::parse(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn converts_to_stroops_exactly() -> Result<(), AmountError> {
        assert_eq!(DecimalAmount::parse("1")?.to_stroops(), 10_000_000);
        assert_eq!(DecimalAmount::parse("0.0000001")?.to_stroops(), 1);
        assert_eq!(DecimalAmount::parse("12.5")?.to_stroops(), 125_000_000);
        assert_eq!(
            DecimalAmount::parse("99999999999.9999999")?.to_stroops(),
            999_999_999_999_999_999
        );
        Ok(())
    }

    #[test]
    fn round_trips_through_json() -> Result<(), serde_json::Error> {
        let amount: DecimalAmount = serde_json::from_str("\"10.50\"")?;
        assert_eq!(serde_json::to_string(&amount)?, "\"10.5\"");
        assert!(
            serde_json::from_str::<DecimalAmount>("10.5").is_err(),
            "numbers are refused"
        );
        Ok(())
    }
}

/// ISO 4217 currency code accepted by the anchor (`CRC`, `ARS`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FiatCurrency(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a 3-letter uppercase ISO 4217 code")]
pub struct CurrencyError(pub String);

impl FiatCurrency {
    pub fn parse(raw: &str) -> Result<Self, CurrencyError> {
        if raw.len() == 3 && raw.bytes().all(|b| b.is_ascii_uppercase()) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(CurrencyError(raw.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// SEP-38 asset id of the currency (`iso4217:CRC`).
    #[must_use]
    pub fn sep38_asset(&self) -> String {
        format!("iso4217:{}", self.0)
    }
}

impl TryFrom<String> for FiatCurrency {
    type Error = CurrencyError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<FiatCurrency> for String {
    fn from(value: FiatCurrency) -> Self {
        value.0
    }
}

#[cfg(test)]
mod currency_tests {
    use super::*;

    #[test]
    fn accepts_iso_codes_only() -> Result<(), CurrencyError> {
        let crc = FiatCurrency::parse("CRC")?;
        assert_eq!(crc.as_str(), "CRC");
        assert_eq!(crc.sep38_asset(), "iso4217:CRC");
        for raw in ["crc", "CR", "CRCC", "C1C", ""] {
            assert!(FiatCurrency::parse(raw).is_err(), "{raw}");
        }
        Ok(())
    }
}
