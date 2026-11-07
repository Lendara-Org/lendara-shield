//! Public 32-byte pool values: note and encryption public keys and ASP
//! leaves, as the pool SDK prints them (`0x` + 64 hex digits).
//!
//! Byte order matters and differs per value (verified against SPP's app,
//! `app/js/blocklist-keys.js`):
//! - the ASP leaf is a field element printed **big-endian**;
//! - the note public key is a field element printed **little-endian**: the
//!   block-list key is its bytes reversed;
//! - the encryption public key is an X25519 key, not a field element.

use std::fmt;

use serde::{Deserialize, Serialize};

/// 32 bytes as `0x`-prefixed hex, normalised to lowercase.
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

    /// Writes `bytes` as `0x` + 64 lowercase hex digits.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        use std::fmt::Write as _;
        let mut out = String::with_capacity(66);
        out.push_str("0x");
        for byte in bytes {
            let _ = write!(out, "{byte:02x}");
        }
        Self(out)
    }

    /// The 32 bytes in the order they are written.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 32] {
        let hex = self.0.as_bytes();
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = (nibble(hex[2 + 2 * i]) << 4) | nibble(hex[3 + 2 * i]);
        }
        out
    }

    /// The value read as a big-endian field element (ASP leaf).
    #[must_use]
    pub fn field_be(&self) -> [u8; 32] {
        self.to_bytes()
    }

    /// The value read as a little-endian field element (note public key),
    /// returned big-endian: the block-list key.
    #[must_use]
    pub fn field_le(&self) -> [u8; 32] {
        let mut bytes = self.to_bytes();
        bytes.reverse();
        bytes
    }

    /// Whether big-endian `bytes` are a canonical BN254 field element.
    #[must_use]
    pub fn is_canonical(bytes: &[u8; 32]) -> bool {
        *bytes < BN254_MODULUS
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Order of the BN254 scalar field the pool's circuits work in, big-endian.
const BN254_MODULUS: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00, 0x00, 0x01,
];

/// Value of one lowercase hex digit (already validated).
fn nibble(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        _ => digit - b'a' + 10,
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
    fn checks_canonical_field_elements() {
        let mut below = BN254_MODULUS;
        below[31] = 0x00;
        assert!(PoolField::is_canonical(&below));
        assert!(!PoolField::is_canonical(&BN254_MODULUS));
        assert!(!PoolField::is_canonical(&[0xff; 32]));
    }

    #[test]
    fn reads_both_byte_orders() -> Result<(), PoolFieldError> {
        let field = PoolField::parse(&format!("0x01{}ff", "ab".repeat(30)))?;
        assert_eq!(field.field_be()[0], 0x01);
        assert_eq!(field.field_be()[31], 0xff);
        assert_eq!(field.field_le()[0], 0xff);
        assert_eq!(field.field_le()[31], 0x01);
        Ok(())
    }

    #[test]
    fn matches_spps_blocklist_key_for_a_real_note_key() -> Result<(), PoolFieldError> {
        // Intake note key from testnet (H5b) and the block-list key SPP's app
        // derives from it: `BigInt(0x + bytes reversed)`.
        let note =
            PoolField::parse("0x5da87d2f37ed7efaa9a5a35227c927df9b19de316e1719983d3b738320f96d09")?;
        let expected =
            PoolField::parse("0x096df92083733b3d9819176e31de199bdf27c92752a3a5a9fa7eed372f7da85d")?;
        assert_eq!(note.field_le(), expected.field_be());
        assert!(PoolField::is_canonical(&note.field_le()));
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
