//! SEP-10 client side: verify the anchor's challenge before signing it.
//! Spec: <https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md>.
//!
//! Signing a challenge proves control of Shield's intake account, so a
//! challenge that does not match the spec exactly is refused, never signed:
//! a valid-looking transaction with a real sequence number would otherwise
//! be executable on the network.

use ed25519_dalek::{Signature as DalekSignature, Signer, SigningKey, Verifier, VerifyingKey};
use stellar_xdr::{
    DecoratedSignature, Memo, MuxedAccount, OperationBody, Preconditions, Signature, SignatureHint,
    TransactionEnvelope, Uint256, VecM,
};

use crate::adapters::soroban::tx::network_id;

/// Clock skew tolerated on the challenge's time bounds.
pub const CLOCK_SKEW_SECS: u64 = 60;
/// SEP-10: the nonce is 48 random bytes, base64-encoded to 64 bytes.
const NONCE_LEN: usize = 64;

/// What a valid challenge must contain.
#[derive(Debug, Clone)]
pub struct Expectations<'a> {
    /// `SIGNING_KEY` from the anchor's `stellar.toml`.
    pub server_key: [u8; 32],
    /// Shield's intake account.
    pub client: [u8; 32],
    pub home_domain: &'a str,
    /// Host (and port) of `WEB_AUTH_ENDPOINT`.
    pub web_auth_domain: &'a str,
    /// The `id` memo requested (the user's customer memo).
    pub memo: Option<u64>,
    pub passphrase: &'a str,
    /// Unix seconds.
    pub now: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChallengeError {
    #[error("not a v1 transaction envelope")]
    Envelope,
    #[error("transaction source is not the anchor's SIGNING_KEY")]
    Source,
    #[error("sequence number is not 0")]
    Sequence,
    #[error("missing or expired time bounds")]
    TimeBounds,
    #[error("memo does not match the request")]
    Memo,
    #[error("first operation is not `<home_domain> auth` from the client account")]
    FirstOperation,
    #[error("the nonce is not 64 bytes")]
    Nonce,
    #[error("web_auth_domain does not match the endpoint")]
    WebAuthDomain,
    #[error("unexpected operation in the challenge")]
    UnexpectedOperation,
    #[error("not signed by the anchor's SIGNING_KEY")]
    ServerSignature,
}

/// Checks a challenge against SEP-10's client verification rules.
pub fn verify(
    envelope: &TransactionEnvelope,
    expect: &Expectations<'_>,
) -> Result<(), ChallengeError> {
    let TransactionEnvelope::Tx(v1) = envelope else {
        return Err(ChallengeError::Envelope);
    };
    let tx = &v1.tx;
    let server = MuxedAccount::Ed25519(Uint256(expect.server_key));
    let client = MuxedAccount::Ed25519(Uint256(expect.client));

    if tx.source_account != server {
        return Err(ChallengeError::Source);
    }
    if tx.seq_num.0 != 0 {
        return Err(ChallengeError::Sequence);
    }
    let Preconditions::Time(bounds) = &tx.cond else {
        return Err(ChallengeError::TimeBounds);
    };
    let (min, max) = (bounds.min_time.0, bounds.max_time.0);
    if max == 0 || expect.now + CLOCK_SKEW_SECS < min || expect.now > max + CLOCK_SKEW_SECS {
        return Err(ChallengeError::TimeBounds);
    }
    let expected_memo = expect.memo.map_or(Memo::None, Memo::Id);
    if tx.memo != expected_memo {
        return Err(ChallengeError::Memo);
    }

    let (first, rest) = tx
        .operations
        .split_first()
        .ok_or(ChallengeError::FirstOperation)?;
    let OperationBody::ManageData(auth) = &first.body else {
        return Err(ChallengeError::FirstOperation);
    };
    let auth_key = format!("{} auth", expect.home_domain);
    if first.source_account.as_ref() != Some(&client)
        || auth.data_name.as_slice() != auth_key.as_bytes()
    {
        return Err(ChallengeError::FirstOperation);
    }
    if auth.data_value.as_ref().map(|v| v.len()) != Some(NONCE_LEN) {
        return Err(ChallengeError::Nonce);
    }

    for op in rest {
        let OperationBody::ManageData(data) = &op.body else {
            return Err(ChallengeError::UnexpectedOperation);
        };
        // Shield does not use client_domain, so every other op must come
        // from the server account.
        if op.source_account.as_ref() != Some(&server) {
            return Err(ChallengeError::UnexpectedOperation);
        }
        if data.data_name.as_slice() == b"web_auth_domain"
            && data.data_value.as_ref().map(|v| v.as_slice())
                != Some(expect.web_auth_domain.as_bytes())
        {
            return Err(ChallengeError::WebAuthDomain);
        }
    }

    let hash = envelope
        .hash(network_id(expect.passphrase))
        .map_err(|_| ChallengeError::Envelope)?;
    let server_key = VerifyingKey::from_bytes(&expect.server_key)
        .map_err(|_| ChallengeError::ServerSignature)?;
    let hint = &expect.server_key[28..32];
    let signed_by_server = v1.signatures.iter().any(|sig| {
        sig.hint.0 == hint
            && DalekSignature::from_slice(sig.signature.0.as_slice())
                .is_ok_and(|s| server_key.verify(&hash, &s).is_ok())
    });
    if !signed_by_server {
        return Err(ChallengeError::ServerSignature);
    }
    Ok(())
}

/// Adds the client's signature to a verified challenge.
pub fn sign(
    envelope: TransactionEnvelope,
    key: &SigningKey,
    passphrase: &str,
) -> Result<TransactionEnvelope, ChallengeError> {
    let hash = envelope
        .hash(network_id(passphrase))
        .map_err(|_| ChallengeError::Envelope)?;
    let TransactionEnvelope::Tx(mut v1) = envelope else {
        return Err(ChallengeError::Envelope);
    };
    let public = key.verifying_key().to_bytes();
    let signature = DecoratedSignature {
        hint: SignatureHint([public[28], public[29], public[30], public[31]]),
        signature: Signature(
            key.sign(&hash)
                .to_bytes()
                .to_vec()
                .try_into()
                .map_err(|_| ChallengeError::Envelope)?,
        ),
    };
    let mut signatures = v1.signatures.to_vec();
    signatures.push(signature);
    v1.signatures = VecM::try_from(signatures).map_err(|_| ChallengeError::Envelope)?;
    Ok(TransactionEnvelope::Tx(v1))
}

#[cfg(test)]
pub(crate) mod tests {
    use stellar_xdr::{
        DataValue, ManageDataOp, Operation, SequenceNumber, String64, TimeBounds, TimePoint,
        Transaction, TransactionExt, TransactionV1Envelope,
    };

    use super::*;

    pub const PASSPHRASE: &str = "Test SDF Network ; September 2015";
    pub const HOME: &str = "anchor.example";
    pub const WEB_AUTH: &str = "auth.anchor.example";
    pub const NOW: u64 = 1_800_000_000;

    pub fn server() -> SigningKey {
        SigningKey::from_bytes(&[1; 32])
    }

    pub fn client() -> SigningKey {
        SigningKey::from_bytes(&[2; 32])
    }

    fn data_op(source: &SigningKey, name: &str, value: &[u8]) -> Operation {
        Operation {
            source_account: Some(MuxedAccount::Ed25519(Uint256(
                source.verifying_key().to_bytes(),
            ))),
            body: OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(name.as_bytes().to_vec()).unwrap_or_default(),
                data_value: Some(DataValue(value.to_vec().try_into().unwrap_or_default())),
            }),
        }
    }

    /// A challenge exactly as the Lendara sandbox builds it, signed by
    /// `server()` after `tweak` edits it.
    pub fn challenge(tweak: impl FnOnce(&mut Transaction)) -> TransactionEnvelope {
        let mut tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(server().verifying_key().to_bytes())),
            fee: 200,
            seq_num: SequenceNumber(0),
            cond: Preconditions::Time(TimeBounds {
                min_time: TimePoint(NOW),
                max_time: TimePoint(NOW + 300),
            }),
            memo: Memo::Id(42),
            operations: vec![
                data_op(&client(), &format!("{HOME} auth"), &[b'n'; 64]),
                data_op(&server(), "web_auth_domain", WEB_AUTH.as_bytes()),
            ]
            .try_into()
            .unwrap_or_default(),
            ext: TransactionExt::V0,
        };
        tweak(&mut tx);
        let unsigned = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx,
            signatures: VecM::default(),
        });
        sign(unsigned.clone(), &server(), PASSPHRASE).unwrap_or(unsigned)
    }

    pub fn expect() -> Expectations<'static> {
        Expectations {
            server_key: server().verifying_key().to_bytes(),
            client: client().verifying_key().to_bytes(),
            home_domain: HOME,
            web_auth_domain: WEB_AUTH,
            memo: Some(42),
            passphrase: PASSPHRASE,
            now: NOW + 10,
        }
    }

    #[test]
    fn accepts_a_valid_challenge() {
        assert_eq!(verify(&challenge(|_| {}), &expect()), Ok(()));
    }

    #[test]
    fn refuses_every_spec_violation() {
        use ChallengeError as E;
        let other = SigningKey::from_bytes(&[9; 32]);
        let cases: Vec<(&str, TransactionEnvelope, E)> = vec![
            (
                "source",
                challenge(|tx| tx.source_account = MuxedAccount::Ed25519(Uint256([9; 32]))),
                E::Source,
            ),
            (
                "sequence",
                challenge(|tx| tx.seq_num = SequenceNumber(1)),
                E::Sequence,
            ),
            (
                "no bounds",
                challenge(|tx| tx.cond = Preconditions::None),
                E::TimeBounds,
            ),
            ("memo", challenge(|tx| tx.memo = Memo::Id(43)), E::Memo),
            ("no memo", challenge(|tx| tx.memo = Memo::None), E::Memo),
            (
                "first op name",
                challenge(|tx| {
                    tx.operations = vec![data_op(&client(), "evil.example auth", &[b'n'; 64])]
                        .try_into()
                        .unwrap_or_default();
                }),
                E::FirstOperation,
            ),
            (
                "first op source",
                challenge(|tx| {
                    tx.operations = vec![data_op(&other, &format!("{HOME} auth"), &[b'n'; 64])]
                        .try_into()
                        .unwrap_or_default();
                }),
                E::FirstOperation,
            ),
            (
                "nonce",
                challenge(|tx| {
                    tx.operations = vec![data_op(&client(), &format!("{HOME} auth"), &[b'n'; 32])]
                        .try_into()
                        .unwrap_or_default();
                }),
                E::Nonce,
            ),
            (
                "web_auth_domain",
                challenge(|tx| {
                    tx.operations = vec![
                        data_op(&client(), &format!("{HOME} auth"), &[b'n'; 64]),
                        data_op(&server(), "web_auth_domain", b"evil.example"),
                    ]
                    .try_into()
                    .unwrap_or_default();
                }),
                E::WebAuthDomain,
            ),
            (
                "extra op from the client",
                challenge(|tx| {
                    tx.operations = vec![
                        data_op(&client(), &format!("{HOME} auth"), &[b'n'; 64]),
                        data_op(&client(), "client_domain", b"x"),
                    ]
                    .try_into()
                    .unwrap_or_default();
                }),
                E::UnexpectedOperation,
            ),
        ];
        for (name, envelope, expected) in cases {
            assert_eq!(verify(&envelope, &expect()), Err(expected), "{name}");
        }
    }

    #[test]
    fn refuses_expired_or_future_challenges() {
        let mut late = expect();
        late.now = NOW + 300 + CLOCK_SKEW_SECS + 1;
        assert_eq!(
            verify(&challenge(|_| {}), &late),
            Err(ChallengeError::TimeBounds)
        );

        let mut early = expect();
        early.now = NOW - CLOCK_SKEW_SECS - 1;
        assert_eq!(
            verify(&challenge(|_| {}), &early),
            Err(ChallengeError::TimeBounds)
        );

        let mut skewed = expect();
        skewed.now = NOW - 30;
        assert_eq!(
            verify(&challenge(|_| {}), &skewed),
            Ok(()),
            "small skew is tolerated"
        );
    }

    #[test]
    fn refuses_a_challenge_not_signed_by_the_anchor() {
        let TransactionEnvelope::Tx(mut v1) = challenge(|_| {}) else {
            return;
        };
        v1.signatures = VecM::default();
        let unsigned = TransactionEnvelope::Tx(v1);
        assert_eq!(
            verify(&unsigned, &expect()),
            Err(ChallengeError::ServerSignature)
        );

        let forged = sign(unsigned, &SigningKey::from_bytes(&[9; 32]), PASSPHRASE);
        assert_eq!(
            forged.map(|e| verify(&e, &expect())),
            Ok(Err(ChallengeError::ServerSignature))
        );
    }

    #[test]
    fn refuses_a_signature_for_another_network() {
        let mut other_network = expect();
        other_network.passphrase = "Public Global Stellar Network ; September 2015";
        assert_eq!(
            verify(&challenge(|_| {}), &other_network),
            Err(ChallengeError::ServerSignature)
        );
    }

    #[test]
    fn signing_keeps_the_server_signature_and_adds_the_client() -> Result<(), ChallengeError> {
        let signed = sign(challenge(|_| {}), &client(), PASSPHRASE)?;
        let TransactionEnvelope::Tx(v1) = &signed else {
            return Err(ChallengeError::Envelope);
        };
        assert_eq!(v1.signatures.len(), 2);
        assert_eq!(verify(&signed, &expect()), Ok(()));
        Ok(())
    }
}
