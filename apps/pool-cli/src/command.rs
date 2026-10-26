//! Command-line parsing and the JSON protocol with the Shield API. Pure.
//!
//! Protocol: one command per process; exactly one JSON object on stdout.
//! Success exits 0; failure exits 1 with `{ "error", "message", "retryable" }`.
//! Secrets come from the environment, never from arguments (`ps` shows them).

use serde::Serialize;
use sha2::{Digest, Sha256};
use stellar_xdr::{Limits, Preconditions, ReadXdr, TransactionEnvelope};

/// A 32-byte value as `0x` + 64 hex digits (the SDK's key format).
pub type Hex32 = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Derives (once) and prints the intake's public pool keys and ASP leaf.
    Keys,
    /// Private balance of the intake in the pool, in base units.
    Balance,
    /// Proves and signs a deposit of `amount` from the intake.
    PrepareDeposit { amount: u128 },
    /// Proves and signs a private transfer of `amount` to raw pool keys.
    PrepareTransfer {
        amount: u128,
        note_key: Hex32,
        encryption_key: Hex32,
    },
    /// Sends a prepared transaction.
    Submit { envelope: String },
    /// Network status of a transaction.
    Status { hash: String },
}

pub const USAGE: &str = "usage: pool-cli <keys | balance | prepare-deposit --amount N | \
     prepare-transfer --amount N --note-key 0x.. --encryption-key 0x.. | \
     submit --envelope XDR | status --hash HEX>";

/// Parses `args` (without the program name).
pub fn parse(args: &[String]) -> Result<Command, CliError> {
    let (name, rest) = args
        .split_first()
        .ok_or_else(|| CliError::usage("missing command"))?;
    let flags = Flags::parse(rest)?;
    let command = match name.as_str() {
        "keys" => Command::Keys,
        "balance" => Command::Balance,
        "prepare-deposit" => Command::PrepareDeposit {
            amount: flags.amount()?,
        },
        "prepare-transfer" => Command::PrepareTransfer {
            amount: flags.amount()?,
            note_key: flags.hex32("--note-key")?,
            encryption_key: flags.hex32("--encryption-key")?,
        },
        "submit" => Command::Submit {
            envelope: flags.required("--envelope")?.to_owned(),
        },
        "status" => {
            let hash = flags.required("--hash")?;
            if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                return Err(CliError::usage("--hash must be 64 lowercase hex digits"));
            }
            Command::Status {
                hash: hash.to_owned(),
            }
        }
        other => return Err(CliError::usage(&format!("unknown command `{other}`"))),
    };
    flags.ensure_all_used(&command)?;
    Ok(command)
}

struct Flags<'a>(Vec<(&'a str, &'a str)>);

impl<'a> Flags<'a> {
    fn parse(args: &'a [String]) -> Result<Self, CliError> {
        let mut pairs = Vec::new();
        let mut iter = args.iter();
        while let Some(flag) = iter.next() {
            if !flag.starts_with("--") {
                return Err(CliError::usage(&format!("unexpected argument `{flag}`")));
            }
            let value = iter
                .next()
                .ok_or_else(|| CliError::usage(&format!("{flag} needs a value")))?;
            if pairs.iter().any(|(f, _)| f == flag) {
                return Err(CliError::usage(&format!("{flag} given twice")));
            }
            pairs.push((flag.as_str(), value.as_str()));
        }
        Ok(Self(pairs))
    }

    fn get(&self, flag: &str) -> Option<&'a str> {
        self.0.iter().find(|(f, _)| *f == flag).map(|(_, v)| *v)
    }

    fn required(&self, flag: &str) -> Result<&'a str, CliError> {
        self.get(flag)
            .ok_or_else(|| CliError::usage(&format!("{flag} is required")))
    }

    fn amount(&self) -> Result<u128, CliError> {
        let raw = self.required("--amount")?;
        match raw.parse::<u128>() {
            Ok(amount) if amount > 0 && raw.bytes().all(|b| b.is_ascii_digit()) => Ok(amount),
            _ => Err(CliError::usage(
                "--amount must be a positive integer in base units",
            )),
        }
    }

    fn hex32(&self, flag: &str) -> Result<Hex32, CliError> {
        let raw = self.required(flag)?;
        let valid = raw
            .strip_prefix("0x")
            .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()));
        if valid {
            Ok(raw.to_ascii_lowercase())
        } else {
            Err(CliError::usage(&format!(
                "{flag} must be 0x + 64 hex digits"
            )))
        }
    }

    fn ensure_all_used(&self, command: &Command) -> Result<(), CliError> {
        let allowed: &[&str] = match command {
            Command::Keys | Command::Balance => &[],
            Command::PrepareDeposit { .. } => &["--amount"],
            Command::PrepareTransfer { .. } => &["--amount", "--note-key", "--encryption-key"],
            Command::Submit { .. } => &["--envelope"],
            Command::Status { .. } => &["--hash"],
        };
        match self.0.iter().find(|(flag, _)| !allowed.contains(flag)) {
            Some((flag, _)) => Err(CliError::usage(&format!("unknown flag {flag}"))),
            None => Ok(()),
        }
    }
}

/// A failure, as written to stdout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliError {
    pub error: &'static str,
    pub message: String,
    /// Whether the same command may succeed later (network, RPC).
    pub retryable: bool,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error, self.message)
    }
}

impl std::error::Error for CliError {}

impl CliError {
    #[must_use]
    pub fn usage(message: &str) -> Self {
        Self {
            error: "invalid_arguments",
            message: format!("{message}. {USAGE}"),
            retryable: false,
        }
    }

    #[must_use]
    pub fn new(error: &'static str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            error,
            message: message.into(),
            retryable,
        }
    }
}

/// A signed transaction ready to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prepared {
    pub hash: String,
    pub envelope_xdr: String,
    /// Upper time bound (unix seconds), or `null` if the transaction has none.
    /// SDK pool transactions have none: use `sequence` to know when it can
    /// no longer land.
    pub valid_until: Option<u64>,
    /// The transaction's sequence number. Once the source account's sequence
    /// reaches it without this hash on-chain, it can never be included.
    pub sequence: i64,
}

/// Hash and time bound of a signed envelope, computed locally so the caller
/// can persist the hash before sending.
pub fn describe(envelope_xdr: &str, passphrase: &str) -> Result<Prepared, CliError> {
    let envelope = TransactionEnvelope::from_xdr_base64(envelope_xdr, Limits::none())
        .map_err(|e| CliError::new("invalid_envelope", e.to_string(), false))?;
    let network_id: [u8; 32] = Sha256::digest(passphrase.as_bytes()).into();
    let hash = envelope
        .hash(network_id)
        .map_err(|e| CliError::new("invalid_envelope", e.to_string(), false))?;
    let TransactionEnvelope::Tx(v1) = &envelope else {
        return Err(CliError::new(
            "invalid_envelope",
            "expected a v1 transaction envelope",
            false,
        ));
    };
    let cond = Some(&v1.tx.cond);
    let valid_until = match cond {
        Some(Preconditions::Time(bounds)) => Some(bounds.max_time.0),
        Some(Preconditions::V2(v2)) => v2.time_bounds.as_ref().map(|b| b.max_time.0),
        _ => None,
    }
    .filter(|max| *max > 0);
    Ok(Prepared {
        hash: hex(&hash),
        envelope_xdr: envelope_xdr.to_owned(),
        valid_until,
        sequence: v1.tx.seq_num.0,
    })
}

/// Lowercase hex of `bytes`, without prefix.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use stellar_xdr::{
        Memo, MuxedAccount, SequenceNumber, TimeBounds, TimePoint, Transaction, TransactionExt,
        TransactionV1Envelope, Uint256, VecM, WriteXdr,
    };

    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_owned()).collect()
    }

    const KEY: &str = "0x5da87d2f37ed7efaa9a5a35227c927df9b19de316e1719983d3b738320f96d09";

    #[test]
    fn parses_every_command() -> Result<(), CliError> {
        assert_eq!(parse(&args(&["keys"]))?, Command::Keys);
        assert_eq!(parse(&args(&["balance"]))?, Command::Balance);
        assert_eq!(
            parse(&args(&["prepare-deposit", "--amount", "93039215"]))?,
            Command::PrepareDeposit { amount: 93_039_215 }
        );
        assert_eq!(
            parse(&args(&[
                "prepare-transfer",
                "--amount",
                "5",
                "--note-key",
                KEY,
                "--encryption-key",
                &KEY.to_uppercase().replacen("0X", "0x", 1)
            ]))?,
            Command::PrepareTransfer {
                amount: 5,
                note_key: KEY.into(),
                encryption_key: KEY.into()
            }
        );
        assert_eq!(
            parse(&args(&["submit", "--envelope", "AAAA"]))?,
            Command::Submit {
                envelope: "AAAA".into()
            }
        );
        assert_eq!(
            parse(&args(&["status", "--hash", &"ab".repeat(32)]))?,
            Command::Status {
                hash: "ab".repeat(32)
            }
        );
        Ok(())
    }

    #[test]
    fn rejects_bad_arguments() {
        for raw in [
            vec![],
            vec!["nope"],
            vec!["prepare-deposit"],
            vec!["prepare-deposit", "--amount", "0"],
            vec!["prepare-deposit", "--amount", "-5"],
            vec!["prepare-deposit", "--amount", "1.5"],
            vec!["prepare-deposit", "--amount"],
            vec!["prepare-deposit", "--amount", "1", "--amount", "2"],
            vec!["prepare-deposit", "--amount", "1", "--secret", "S..."],
            vec![
                "prepare-transfer",
                "--amount",
                "1",
                "--note-key",
                "0x12",
                "--encryption-key",
                KEY,
            ],
            vec!["status", "--hash", "XYZ"],
            vec!["keys", "extra"],
        ] {
            let parsed = parse(&args(&raw));
            assert!(
                matches!(&parsed, Err(e) if e.error == "invalid_arguments" && !e.retryable),
                "{raw:?}: {parsed:?}"
            );
        }
    }

    fn envelope(cond: Preconditions) -> Result<String, stellar_xdr::Error> {
        TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: Transaction {
                source_account: MuxedAccount::Ed25519(Uint256([3; 32])),
                fee: 100,
                seq_num: SequenceNumber(7),
                cond,
                memo: Memo::None,
                operations: VecM::default(),
                ext: TransactionExt::V0,
            },
            signatures: VecM::default(),
        })
        .to_xdr_base64(Limits::none())
    }

    #[test]
    fn describes_hash_and_time_bound() -> Result<(), Box<dyn std::error::Error>> {
        let passphrase = "Test SDF Network ; September 2015";
        let xdr = envelope(Preconditions::Time(TimeBounds {
            min_time: TimePoint(0),
            max_time: TimePoint(1_800_000_060),
        }))?;
        let prepared = describe(&xdr, passphrase)?;
        assert_eq!(prepared.hash.len(), 64);
        assert_eq!(prepared.valid_until, Some(1_800_000_060));
        assert_eq!(prepared.sequence, 7);
        assert_eq!(prepared.envelope_xdr, xdr);

        let other_network = describe(&xdr, "Public Global Stellar Network ; September 2015")?;
        assert_ne!(
            other_network.hash, prepared.hash,
            "the hash commits to the network"
        );

        let unbounded = describe(&envelope(Preconditions::None)?, passphrase)?;
        assert_eq!(unbounded.valid_until, None);
        Ok(())
    }

    #[test]
    fn rejects_garbage_envelopes() {
        assert!(matches!(describe("not xdr", "x"), Err(e) if e.error == "invalid_envelope"));
    }

    #[test]
    fn errors_serialize_with_the_protocol_fields() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(CliError::new("chain", "rpc down", true))?;
        assert_eq!(
            json,
            serde_json::json!({"error": "chain", "message": "rpc down", "retryable": true})
        );
        Ok(())
    }
}
