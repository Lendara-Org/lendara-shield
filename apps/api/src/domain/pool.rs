//! Values of the pool's zero-knowledge field: public note and encryption
//! keys and ASP leaves.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A 32-byte field element as `0x`-prefixed hex, normalised to lowercase.
///
/// Only **public** values ever take this type (keys, leaves); spending keys
/// never reach Shield.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PoolField(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a 0x-prefixed 32-byte hex value")]
pub struct PoolFieldError(pub String);

impl PoolField {
    pub fn parse(raw: &str) -> Result<Self, PoolFieldError> {
        let hex = raw
            .strip_prefix("0x")
            .filter(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| PoolFieldError(raw.to_owned()))?;
        Ok(Self(format!("0x{}", hex.to_ascii_lowercase())))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PoolField {
    type Error = PoolFieldError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<PoolField> for String {
    fn from(value: PoolField) -> Self {
        value.0
    }
}

impl fmt::Display for PoolField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEAF: &str = "0x05a8c36ec6273fca4fed02573a4030279b2ec26f79ebf9cc784d8e6313f8fd74";

    #[test]
    fn accepts_and_normalises_to_lowercase() -> Result<(), PoolFieldError> {
        assert_eq!(PoolField::parse(LEAF)?.as_str(), LEAF);
        assert_eq!(
            PoolField::parse(&LEAF.to_uppercase().replacen("0X", "0x", 1))?.as_str(),
            LEAF
        );
        Ok(())
    }

    #[test]
    fn displays_and_round_trips_through_json() -> Result<(), Box<dyn std::error::Error>> {
        let field = PoolField::parse(LEAF)?;
        assert_eq!(field.to_string(), LEAF);
        let json = serde_json::to_string(&field)?;
        assert_eq!(json, format!("\"{LEAF}\""));
        assert_eq!(serde_json::from_str::<PoolField>(&json)?, field);
        assert!(serde_json::from_str::<PoolField>("\"0x12\"").is_err());
        Ok(())
    }

    #[test]
    fn rejects_malformed_values() {
        for raw in [
            "",
            "0x",
            &LEAF[2..],                    // no prefix
            &LEAF[..65],                   // too short
            &format!("{LEAF}00"),          // too long
            &LEAF.replacen('5', "g", 1),   // not hex
            &LEAF.replacen("0x", "0X", 1), // uppercase prefix
        ] {
            assert!(PoolField::parse(raw).is_err(), "{raw}");
        }
    }
}
