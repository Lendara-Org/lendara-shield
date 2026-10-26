//! Stellar addresses, validated with the official `stellar-strkey` codec
//! (base32 alphabet, version byte and CRC16 checksum).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Why a string is not the expected kind of Stellar address.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StrkeyError {
    #[error("`{0}` is not a valid G... account address")]
    Account(String),
    #[error("`{0}` is not a valid C... contract address")]
    Contract(String),
    #[error("`{0}` is not a 64-character lowercase hex transaction hash")]
    TxHash(String),
}

/// A Stellar account public key (`G...`). Muxed (`M...`) accounts are not
/// accepted: Shield identifies users by their plain account.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AccountId(String);

impl AccountId {
    pub fn parse(raw: &str) -> Result<Self, StrkeyError> {
        stellar_strkey::ed25519::PublicKey::from_string(raw)
            .map(|_| Self(raw.to_owned()))
            .map_err(|_| StrkeyError::Account(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Soroban contract address (`C...`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContractId(String);

impl ContractId {
    pub fn parse(raw: &str) -> Result<Self, StrkeyError> {
        stellar_strkey::Contract::from_string(raw)
            .map(|_| Self(raw.to_owned()))
            .map_err(|_| StrkeyError::Contract(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Stellar transaction hash: 32 bytes as lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TxHash(String);

impl TxHash {
    pub fn parse(raw: &str) -> Result<Self, StrkeyError> {
        if raw.len() == 64 && raw.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(StrkeyError::TxHash(raw.to_owned()))
        }
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        use std::fmt::Write;
        let hex = bytes.iter().fold(String::with_capacity(64), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        });
        Self(hex)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! string_conversions {
    ($ty:ty) => {
        impl TryFrom<String> for $ty {
            type Error = StrkeyError;
            fn try_from(raw: String) -> Result<Self, Self::Error> {
                Self::parse(&raw)
            }
        }

        impl From<$ty> for String {
            fn from(value: $ty) -> Self {
                value.0
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_conversions!(AccountId);
string_conversions!(ContractId);
string_conversions!(TxHash);

#[cfg(test)]
mod tests {
    use super::*;

    // Real testnet addresses from the H1 spike (public).
    const ACCOUNT: &str = "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY";
    const CONTRACT: &str = "CCAEA4MKORUJV3G3V47SKWLJKQZBYGGFU42JSCXNUIJIZMNN2MSDLFMO";

    #[test]
    fn accepts_valid_addresses() -> Result<(), StrkeyError> {
        assert_eq!(AccountId::parse(ACCOUNT)?.as_str(), ACCOUNT);
        assert_eq!(ContractId::parse(CONTRACT)?.as_str(), CONTRACT);
        Ok(())
    }

    #[test]
    fn rejects_a_bad_checksum() {
        let mut corrupted = ACCOUNT.to_owned();
        corrupted.replace_range(55..56, "A");
        assert_eq!(
            AccountId::parse(&corrupted),
            Err(StrkeyError::Account(corrupted.clone()))
        );
    }

    #[test]
    fn rejects_the_wrong_kind_of_address() {
        assert!(AccountId::parse(CONTRACT).is_err());
        assert!(ContractId::parse(ACCOUNT).is_err());
    }

    #[test]
    fn rejects_garbage_and_lowercase() {
        for raw in ["", "G", "hello", &ACCOUNT.to_lowercase()] {
            assert!(AccountId::parse(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn parses_transaction_hashes() -> Result<(), StrkeyError> {
        let hash = "770f0c7954347bfd2663f0c7b4b268c05bcfdc5205f702253adf60e0498b17b6";
        assert_eq!(TxHash::parse(hash)?.as_str(), hash);
        assert!(TxHash::parse(&hash.to_uppercase()).is_err());
        assert!(TxHash::parse(&hash[1..]).is_err());
        assert_eq!(TxHash::from_bytes([0xab; 32]).as_str(), "ab".repeat(32));
        Ok(())
    }

    #[test]
    fn round_trips_through_json() -> Result<(), serde_json::Error> {
        let json = format!("\"{ACCOUNT}\"");
        let account: AccountId = serde_json::from_str(&json)?;
        assert_eq!(serde_json::to_string(&account)?, json);
        assert!(serde_json::from_str::<AccountId>("\"GNOPE\"").is_err());
        Ok(())
    }
}
