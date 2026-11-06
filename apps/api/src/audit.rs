//! The auditor's view of the pool and the public view of a transaction, for
//! the side-by-side comparison. The Global View Key never leaves the
//! `pool-cli` subprocess; this module only sees decrypted notes.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use sqlx::PgPool;

use crate::{
    domain::{AccountId, PoolField, TxHash, amount::format_stroops},
    ports::{AuditedNote, Explorer, PoolAuditor, PublicTx},
    store::{StoreError, users},
};

/// A pool operation by its public amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxKind {
    Deposit,
    Transfer,
    Withdraw,
}

impl TxKind {
    /// The pool balances: `ext_amount = Σ outputs − Σ inputs`.
    #[must_use]
    pub fn of(ext_stroops: i128) -> Self {
        match ext_stroops.signum() {
            1 => Self::Deposit,
            -1 => Self::Withdraw,
            _ => Self::Transfer,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deposit => "deposit",
            Self::Transfer => "transfer",
            Self::Withdraw => "withdraw",
        }
    }
}

/// Who owns a decrypted note, as Shield knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerKind {
    /// A Shield user (its account is known).
    User,
    /// Shield's omnibus account: it holds a ramp deposit between the
    /// anchor's payment and the private transfer to the user. Without this
    /// label the auditor would see it as an unknown key.
    Intake,
    /// Neither, e.g. a transfer to someone outside Shield.
    Unknown,
}

impl OwnerKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Intake => "intake",
            Self::Unknown => "unknown",
        }
    }
}

/// A decrypted note attributed to a Shield user when possible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditNoteView {
    /// The owner's note public key as users register it.
    pub owner_note_public_key: PoolField,
    pub owner_kind: OwnerKind,
    /// `Some` exactly when `owner_kind` is [`OwnerKind::User`].
    pub owner_stellar_account: Option<AccountId>,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditTxView {
    pub ledger: u32,
    pub tx_hash: Option<TxHash>,
    pub kind: TxKind,
    pub ext_amount: String,
    pub inputs: Vec<AuditNoteView>,
    pub outputs: Vec<AuditNoteView>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("the auditor is not configured")]
    NotConfigured,
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, Clone)]
pub struct AuditService {
    db: PgPool,
    auditor: Option<Arc<dyn PoolAuditor>>,
    explorer: Option<Arc<dyn Explorer>>,
    /// Ledger → the pool transaction in it. Ledgers never change.
    hashes: Arc<Mutex<HashMap<u32, Option<TxHash>>>>,
    /// The intake's note key, once known. It never changes for a wallet.
    intake_key: Arc<Mutex<Option<PoolField>>>,
}

impl AuditService {
    #[must_use]
    pub fn new(
        db: PgPool,
        auditor: Option<Arc<dyn PoolAuditor>>,
        explorer: Option<Arc<dyn Explorer>>,
    ) -> Self {
        Self {
            db,
            auditor,
            explorer,
            hashes: Arc::default(),
            intake_key: Arc::default(),
        }
    }

    /// The newest `limit` pool transactions, decrypted and attributed.
    pub async fn transactions(&self, limit: usize) -> Result<Vec<AuditTxView>, AuditError> {
        let auditor = self.auditor.as_ref().ok_or(AuditError::NotConfigured)?;
        let txs = auditor
            .audit(limit)
            .await
            .map_err(|e| AuditError::Unavailable(e.message))?;

        let owners: Vec<PoolField> = txs
            .iter()
            .flat_map(|tx| tx.inputs.iter().chain(&tx.outputs))
            .map(registered_key)
            .collect();
        let accounts = users::accounts_by_note_keys(&self.db, &owners).await?;
        let intake = self.intake_key(auditor.as_ref()).await;

        let mut views = Vec::with_capacity(txs.len());
        for tx in txs {
            let stroops =
                |notes: &[AuditedNote]| notes.iter().map(|n| i128::from(n.stroops)).sum::<i128>();
            let ext = stroops(&tx.outputs) - stroops(&tx.inputs);
            let attribute = |notes: &[AuditedNote]| -> Vec<AuditNoteView> {
                notes
                    .iter()
                    .map(|note| {
                        let key = registered_key(note);
                        // The intake's key, confirmed by its own wallet, wins
                        // over user records: onboarding proves the account,
                        // not that the note key is its own, so anyone could
                        // register the intake's public key as theirs.
                        let (owner_kind, account) = if intake.as_ref() == Some(&key) {
                            (OwnerKind::Intake, None)
                        } else {
                            match accounts.get(&key) {
                                Some(account) => (OwnerKind::User, Some(account.clone())),
                                None => (OwnerKind::Unknown, None),
                            }
                        };
                        AuditNoteView {
                            owner_kind,
                            owner_stellar_account: account,
                            owner_note_public_key: key,
                            amount: format_stroops(i128::from(note.stroops)),
                        }
                    })
                    .collect()
            };
            views.push(AuditTxView {
                ledger: tx.ledger,
                tx_hash: self.hash_in(tx.ledger).await,
                kind: TxKind::of(ext),
                ext_amount: format_stroops(ext),
                inputs: attribute(&tx.inputs),
                outputs: attribute(&tx.outputs),
            });
        }
        Ok(views)
    }

    /// What anyone sees about `hash`; `None` if the network does not know it.
    pub async fn public(&self, hash: &TxHash) -> Result<Option<PublicTx>, AuditError> {
        let explorer = self.explorer.as_ref().ok_or(AuditError::NotConfigured)?;
        explorer
            .transaction(hash)
            .await
            .map_err(|e| AuditError::Unavailable(e.0))
    }

    /// The intake's note key. A failed lookup only labels its notes
    /// `unknown` for this request; it is retried on the next one.
    async fn intake_key(&self, auditor: &dyn PoolAuditor) -> Option<PoolField> {
        if let Some(known) = self.known_intake_key().clone() {
            return Some(known);
        }
        match auditor.intake_note_key().await {
            Ok(key) => {
                *self.known_intake_key() = Some(key.clone());
                Some(key)
            }
            Err(err) => {
                tracing::warn!(code = %err.code, message = %err.message, "could not read the intake's note key");
                None
            }
        }
    }

    fn known_intake_key(&self) -> std::sync::MutexGuard<'_, Option<PoolField>> {
        self.intake_key
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The pool transaction in `ledger` when exactly one is there. A lookup
    /// failure only leaves the hash out; it is retried on the next request.
    async fn hash_in(&self, ledger: u32) -> Option<TxHash> {
        if let Some(cached) = self.cache().get(&ledger) {
            return cached.clone();
        }
        let explorer = self.explorer.as_ref()?;
        match explorer.pool_transactions_in_ledger(ledger).await {
            Ok(hashes) => {
                let hash = match hashes.as_slice() {
                    [only] => Some(only.clone()),
                    _ => None,
                };
                // An empty answer may be Horizon lagging behind: ask again later.
                if !hashes.is_empty() {
                    self.cache().insert(ledger, hash.clone());
                }
                hash
            }
            Err(err) => {
                tracing::warn!(ledger, error = %err.0, "could not resolve the pool transaction of a ledger");
                None
            }
        }
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Option<TxHash>>> {
        self.hashes.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The SDK's audit prints a note key as a big-endian field element; users
/// register it little-endian (the reverse bytes).
fn registered_key(note: &AuditedNote) -> PoolField {
    PoolField::from_bytes(note.owner.field_le())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_follows_the_sign_of_ext_amount() {
        assert_eq!(TxKind::of(93_039_215), TxKind::Deposit);
        assert_eq!(TxKind::of(0), TxKind::Transfer);
        assert_eq!(TxKind::of(-5_000_000), TxKind::Withdraw);
    }

    #[test]
    fn maps_audit_keys_to_registered_keys() -> Result<(), crate::domain::PoolFieldError> {
        // Bob, verified on testnet: registered 0xa2a6…3824, audited 0x2438…a6a2.
        let note = AuditedNote {
            owner: PoolField::parse(
                "0x24382f33fe99fcea57632e6dd4ba6eca4d7053785ff10b71e90577541cd7a6a2",
            )?,
            stroops: 5_000_000,
        };
        assert_eq!(
            registered_key(&note).as_str(),
            "0xa2a6d71c547705e9710bf15f7853704dca6ebad46d2e6357eafc99fe332f3824"
        );
        Ok(())
    }
}
