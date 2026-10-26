//! Adapters: implementations of the [`ports`](crate::ports) against real
//! services.

pub mod anchor;
pub mod horizon;
pub mod soroban;

/// The `G...` account of a secret seed, without exposing the seed.
#[must_use]
pub fn account_of(secret: &secrecy::SecretString) -> Option<crate::domain::AccountId> {
    use secrecy::ExposeSecret;
    let seed = stellar_strkey::ed25519::PrivateKey::from_string(secret.expose_secret()).ok()?;
    let public = ed25519_dalek::SigningKey::from_bytes(&seed.0)
        .verifying_key()
        .to_bytes();
    crate::domain::AccountId::parse(
        stellar_strkey::ed25519::PublicKey(public)
            .to_string()
            .as_str(),
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::account_of;

    #[test]
    fn derives_the_account_of_a_seed() {
        let seed = stellar_strkey::ed25519::PrivateKey([2; 32])
            .as_unredacted()
            .to_string()
            .to_string();
        let account = account_of(&SecretString::from(seed));
        assert!(account.is_some_and(|a| a.as_str().starts_with('G')));
        assert!(account_of(&SecretString::from("SNOPE")).is_none());
    }
}
