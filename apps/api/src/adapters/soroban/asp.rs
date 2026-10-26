//! [`AspContract`] on Soroban: writes to SPP's ASP trees signed by the
//! operator (the trees' admin).
//!
//! - Allow: `asp_membership.insert_leaf(leaf)`.
//! - Block: `asp_non_membership.insert_leaf(note_key, note_key)` (SPP app
//!   convention: key and value are both the note public key).
//! - Unblock: `asp_non_membership.delete_leaf(note_key)`.

use std::{fmt, sync::Arc, time::Duration};

use async_trait::async_trait;
use ed25519_dalek::SigningKey;
use secrecy::{ExposeSecret, SecretString};
use stellar_xdr::{Limits, ReadXdr, ScVal, TransactionEnvelope, WriteXdr};
use tokio::sync::Mutex;

use super::{
    rpc::{GetStatus, RpcError, SendStatus, SimulationOutcome, SorobanRpc},
    tx,
};
use crate::{
    asp::AspAction,
    domain::{ContractId, TxHash},
    ports::{AspContract, AspKeys, ChainError, Clock, PreparedTx, TxStatus},
};

/// How long a prepared transaction stays valid (its upper time bound).
pub const DEFAULT_VALIDITY: Duration = Duration::from_secs(60);

#[derive(Debug, thiserror::Error)]
pub enum SorobanAspError {
    #[error("SHIELD_OPERATOR_SECRET is not a valid S... secret seed")]
    InvalidSecret,
    #[error(transparent)]
    Rpc(#[from] RpcError),
}

pub struct SorobanAspConfig {
    pub rpc_url: String,
    pub network_passphrase: String,
    pub operator_secret: SecretString,
    pub asp_membership: ContractId,
    pub asp_non_membership: ContractId,
    pub validity: Duration,
}

impl fmt::Debug for SorobanAspConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SorobanAspConfig")
            .field("rpc_url", &self.rpc_url)
            .field("asp_membership", &self.asp_membership)
            .field("asp_non_membership", &self.asp_non_membership)
            .finish_non_exhaustive()
    }
}

pub struct SorobanAspContract {
    rpc: SorobanRpc,
    passphrase: String,
    operator: SigningKey,
    asp_membership: ContractId,
    asp_non_membership: ContractId,
    validity: chrono::Duration,
    clock: Arc<dyn Clock>,
    /// Last sequence number this process signed. The network only learns a
    /// sequence once the transaction lands, so consecutive prepares must not
    /// reuse it. Reset when a transaction is refused (its number is unused).
    /// A prepared transaction that is never sent leaves a gap: the next send
    /// is refused with a bad sequence, which resets this and the service's
    /// bounded retry recovers.
    last_sequence: Mutex<Option<i64>>,
}

impl fmt::Debug for SorobanAspContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SorobanAspContract")
            .field("asp_membership", &self.asp_membership)
            .field("asp_non_membership", &self.asp_non_membership)
            .finish_non_exhaustive()
    }
}

impl SorobanAspContract {
    pub fn new(config: SorobanAspConfig, clock: Arc<dyn Clock>) -> Result<Self, SorobanAspError> {
        let seed = stellar_strkey::ed25519::PrivateKey::from_string(
            config.operator_secret.expose_secret(),
        )
        .map_err(|_| SorobanAspError::InvalidSecret)?;
        Ok(Self {
            rpc: SorobanRpc::new(config.rpc_url)?,
            passphrase: config.network_passphrase,
            operator: SigningKey::from_bytes(&seed.0),
            asp_membership: config.asp_membership,
            asp_non_membership: config.asp_non_membership,
            validity: chrono::Duration::from_std(config.validity)
                .unwrap_or_else(|_| chrono::Duration::seconds(60)),
            clock,
            last_sequence: Mutex::new(None),
        })
    }

    /// Public key (`G...`) of the operator that signs ASP writes.
    #[must_use]
    pub fn operator_address(&self) -> String {
        stellar_strkey::ed25519::PublicKey(self.operator.verifying_key().to_bytes())
            .to_string()
            .to_string()
    }

    /// Contract, function and arguments of `action`. The leaf is read
    /// big-endian; the note key little-endian (block-list key = its bytes
    /// reversed, as SPP's app does).
    fn call(&self, action: AspAction, keys: &AspKeys) -> (&ContractId, &'static str, Vec<ScVal>) {
        let block_key = tx::u256(keys.note_public_key.field_le());
        match action {
            AspAction::Allow => (
                &self.asp_membership,
                "insert_leaf",
                vec![tx::u256(keys.asp_leaf.field_be())],
            ),
            AspAction::Block => (
                &self.asp_non_membership,
                "insert_leaf",
                vec![block_key.clone(), block_key],
            ),
            AspAction::Unblock => (&self.asp_non_membership, "delete_leaf", vec![block_key]),
        }
    }
}

#[async_trait]
impl AspContract for SorobanAspContract {
    async fn prepare(&self, action: AspAction, keys: &AspKeys) -> Result<PreparedTx, ChainError> {
        let source = self.operator.verifying_key().to_bytes();
        let valid_until = self.clock.now() + self.validity;
        let max_time =
            u64::try_from(valid_until.timestamp()).map_err(|e| unavailable(e.to_string()))?;
        let (contract, function, args) = self.call(action, keys);

        // Held until signing, so two prepares never take the same number.
        let mut last = self.last_sequence.lock().await;
        let on_chain = self
            .rpc
            .account_sequence(&source)
            .await
            .map_err(|e| rpc_unavailable(&e))?;
        let sequence = on_chain.max(last.unwrap_or(i64::MIN)) + 1;

        let draft = tx::build_invoke(&source, sequence, contract, function, args, max_time)
            .map_err(|e| unavailable(e.to_string()))?;
        let simulation = match self
            .rpc
            .simulate(&draft)
            .await
            .map_err(|e| rpc_unavailable(&e))?
        {
            SimulationOutcome::Ok(simulation) => simulation,
            SimulationOutcome::Failed(reason) => return Err(ChainError::Rejected(reason)),
        };
        let assembled =
            tx::assemble(&draft, &simulation).map_err(|e| unavailable(e.to_string()))?;
        let (signed, hash) = tx::sign(assembled, &self.operator, &self.passphrase)
            .map_err(|e| unavailable(e.to_string()))?;
        *last = Some(sequence);

        Ok(PreparedTx {
            hash: TxHash::from_bytes(hash),
            envelope_xdr: signed
                .to_xdr_base64(Limits::none())
                .map_err(|e| unavailable(e.to_string()))?,
            valid_until,
        })
    }

    async fn submit(&self, prepared: &PreparedTx) -> Result<(), ChainError> {
        let envelope = TransactionEnvelope::from_xdr_base64(&prepared.envelope_xdr, Limits::none())
            .map_err(|e| ChainError::Rejected(format!("invalid envelope: {e}")))?;
        match self
            .rpc
            .send(&envelope)
            .await
            .map_err(|e| rpc_unavailable(&e))?
        {
            SendStatus::Pending | SendStatus::Duplicate => Ok(()),
            SendStatus::TryAgainLater => {
                Err(unavailable("RPC queue full (TRY_AGAIN_LATER)".into()))
            }
            SendStatus::Error(result) => {
                // Refused before inclusion: its sequence number stays free.
                *self.last_sequence.lock().await = None;
                Err(ChainError::Rejected(result))
            }
        }
    }

    async fn status(&self, hash: &TxHash) -> Result<TxStatus, ChainError> {
        Ok(
            match self
                .rpc
                .get_transaction(hash.as_str())
                .await
                .map_err(|e| rpc_unavailable(&e))?
            {
                GetStatus::Success => TxStatus::Success,
                GetStatus::Failed(result) => TxStatus::Failed(result),
                GetStatus::NotFound => TxStatus::NotFound,
            },
        )
    }
}

fn unavailable(reason: String) -> ChainError {
    ChainError::Unavailable(reason)
}

fn rpc_unavailable(err: &RpcError) -> ChainError {
    ChainError::Unavailable(err.to_string())
}
