//! Proof that whoever onboards an account controls it: a SEP-53 signature
//! by the account over the exact keys it registers. Pure.
//!
//! SEP-53: `ed25519_sign(sha256("Stellar Signed Message:\n" + message))`,
//! base64-encoded; Freighter's `signMessage` produces it.
//! <https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0053.md>

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::domain::{AccountId, PoolField};

const SEP53_PREFIX: &str = "Stellar Signed Message:\n";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OwnershipError {
    #[error("signature must be a base64-encoded 64-byte Ed25519 signature")]
    Malformed,
    #[error("the signature is not the account's signature over these keys")]
    Invalid,
}

/// The text the user signs to onboard. Keys in lowercase `0x` hex, as the
/// API stores them; the web builds the same text.
#[must_use]
pub fn onboarding_message(
    account: &AccountId,
    note_public_key: &PoolField,
    encryption_public_key: &PoolField,
    asp_leaf: &PoolField,
) -> String {
    format!(
        "Lendara Shield onboarding\n\
         network: testnet\n\
         account: {account}\n\
         note_public_key: {note}\n\
         encryption_public_key: {encryption}\n\
         asp_leaf: {leaf}",
        account = account.as_str(),
        note = note_public_key.as_str(),
        encryption = encryption_public_key.as_str(),
        leaf = asp_leaf.as_str(),
    )
}

/// The text the user signs to start a ramp intent. `fiat_currency` and
/// `amount` exactly as sent in the request; `request_id` in lowercase
/// hyphenated form and `issued_at` in Unix seconds make every signature
/// single-use (see `POST /v1/intents`). The web builds the same text.
#[must_use]
pub fn intent_message(
    account: &AccountId,
    fiat_currency: &str,
    amount: &str,
    request_id: &uuid::Uuid,
    issued_at: i64,
) -> String {
    format!(
        "Lendara Shield intent\n\
         network: testnet\n\
         account: {account}\n\
         fiat_currency: {fiat_currency}\n\
         amount: {amount}\n\
         request_id: {request_id}\n\
         issued_at: {issued_at}",
        account = account.as_str(),
        request_id = request_id.hyphenated(),
    )
}

/// Checks that `signature` is `account`'s SEP-53 signature of `message`.
pub fn verify(account: &AccountId, message: &str, signature: &str) -> Result<(), OwnershipError> {
    let bytes = STANDARD
        .decode(signature.trim())
        .map_err(|_| OwnershipError::Malformed)?;
    let signature = Signature::from_slice(&bytes).map_err(|_| OwnershipError::Malformed)?;
    let public = stellar_strkey::ed25519::PublicKey::from_string(account.as_str())
        .map_err(|_| OwnershipError::Invalid)?;
    let key = VerifyingKey::from_bytes(&public.0).map_err(|_| OwnershipError::Invalid)?;
    let digest = Sha256::digest(format!("{SEP53_PREFIX}{message}").as_bytes());
    key.verify_strict(&digest, &signature)
        .map_err(|_| OwnershipError::Invalid)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn account(key: &SigningKey) -> Result<AccountId, Box<dyn std::error::Error>> {
        let strkey = stellar_strkey::ed25519::PublicKey(key.verifying_key().to_bytes());
        Ok(AccountId::parse(&format!("{strkey}"))?)
    }

    fn sep53(key: &SigningKey, message: &str) -> String {
        let digest = Sha256::digest(format!("{SEP53_PREFIX}{message}").as_bytes());
        STANDARD.encode(key.sign(&digest).to_bytes())
    }

    fn field(b: u8) -> PoolField {
        PoolField::from_bytes([b; 32])
    }

    fn message(account: &AccountId) -> String {
        onboarding_message(account, &field(0xa2), &field(0x01), &field(0x05))
    }

    #[test]
    fn the_message_names_the_account_and_every_key() -> Result<(), Box<dyn std::error::Error>> {
        let alice = AccountId::parse("GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC")?;
        // The web builds the same text (apps/web/src/lib/onboarding.ts).
        assert_eq!(
            message(&alice),
            format!(
                "Lendara Shield onboarding\nnetwork: testnet\n\
                 account: GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC\n\
                 note_public_key: 0x{}\nencryption_public_key: 0x{}\nasp_leaf: 0x{}",
                "a2".repeat(32),
                "01".repeat(32),
                "05".repeat(32)
            )
        );
        Ok(())
    }

    #[test]
    fn the_intent_message_binds_the_amount_and_a_single_use_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let alice = AccountId::parse("GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC")?;
        let request_id = uuid::Uuid::parse_str("0194F3A2-7C1E-7D3B-9A4F-2B6C8E1D5F70")?;
        // The web builds the same text (apps/web/src/lib/intent-request.ts).
        assert_eq!(
            intent_message(&alice, "CRC", "5000", &request_id, 1_793_016_000),
            "Lendara Shield intent\nnetwork: testnet\n\
             account: GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC\n\
             fiat_currency: CRC\namount: 5000\n\
             request_id: 0194f3a2-7c1e-7d3b-9a4f-2b6c8e1d5f70\nissued_at: 1793016000"
        );
        Ok(())
    }

    #[test]
    fn an_intent_signature_does_not_cover_another_amount() -> Result<(), Box<dyn std::error::Error>>
    {
        let key = SigningKey::from_bytes(&[7; 32]);
        let alice = account(&key)?;
        let id = uuid::Uuid::from_u128(1);
        let signed = sep53(&key, &intent_message(&alice, "CRC", "5000", &id, 1));
        assert_eq!(
            verify(
                &alice,
                &intent_message(&alice, "CRC", "5000", &id, 1),
                &signed
            ),
            Ok(())
        );
        assert_eq!(
            verify(
                &alice,
                &intent_message(&alice, "CRC", "50000", &id, 1),
                &signed
            ),
            Err(OwnershipError::Invalid)
        );
        // An onboarding signature cannot be reused as an intent signature.
        let onboarding = sep53(&key, &message(&alice));
        assert_eq!(
            verify(
                &alice,
                &intent_message(&alice, "CRC", "5000", &id, 1),
                &onboarding
            ),
            Err(OwnershipError::Invalid)
        );
        Ok(())
    }

    #[test]
    fn accepts_the_accounts_sep53_signature() -> Result<(), Box<dyn std::error::Error>> {
        let key = SigningKey::from_bytes(&[7; 32]);
        let alice = account(&key)?;
        assert_eq!(
            verify(&alice, &message(&alice), &sep53(&key, &message(&alice))),
            Ok(())
        );
        Ok(())
    }

    #[test]
    fn rejects_another_accounts_signature() -> Result<(), Box<dyn std::error::Error>> {
        let (owner, attacker) = (
            SigningKey::from_bytes(&[7; 32]),
            SigningKey::from_bytes(&[8; 32]),
        );
        let victim = account(&owner)?;
        // The attacker signs the victim's account with their own key.
        assert_eq!(
            verify(
                &victim,
                &message(&victim),
                &sep53(&attacker, &message(&victim))
            ),
            Err(OwnershipError::Invalid)
        );
        Ok(())
    }

    #[test]
    fn rejects_a_signature_over_other_keys() -> Result<(), Box<dyn std::error::Error>> {
        let key = SigningKey::from_bytes(&[7; 32]);
        let alice = account(&key)?;
        let signed = sep53(&key, &message(&alice));
        let swapped = onboarding_message(&alice, &field(0xbb), &field(0x01), &field(0x05));
        assert_eq!(
            verify(&alice, &swapped, &signed),
            Err(OwnershipError::Invalid)
        );
        Ok(())
    }

    #[test]
    fn rejects_a_raw_signature_without_the_sep53_prefix() -> Result<(), Box<dyn std::error::Error>>
    {
        let key = SigningKey::from_bytes(&[7; 32]);
        let alice = account(&key)?;
        let raw = STANDARD.encode(key.sign(message(&alice).as_bytes()).to_bytes());
        assert_eq!(
            verify(&alice, &message(&alice), &raw),
            Err(OwnershipError::Invalid)
        );
        Ok(())
    }

    #[test]
    fn rejects_malformed_signatures() -> Result<(), Box<dyn std::error::Error>> {
        let alice = account(&SigningKey::from_bytes(&[7; 32]))?;
        for bad in ["", "not base64!", "AAAA", &STANDARD.encode([0u8; 63])] {
            assert_eq!(
                verify(&alice, &message(&alice), bad),
                Err(OwnershipError::Malformed),
                "{bad}"
            );
        }
        Ok(())
    }
}
