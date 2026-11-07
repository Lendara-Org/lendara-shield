//! Pure transaction building: no I/O, fully unit-tested.
//!
//! Mirrors the JS SDK's `assembleTransaction` and SPP's Rust SDK: build an
//! `InvokeHostFunction` transaction, merge the RPC simulation into it (fees,
//! footprint, auth), then sign it.

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use stellar_xdr::{
    ContractDataDurability, ContractId, DecoratedSignature, Hash, HostFunction, InvokeContractArgs,
    InvokeHostFunctionOp, LedgerFootprint, LedgerKey, Memo, MuxedAccount, Operation, OperationBody,
    Preconditions, ScAddress, ScSymbol, ScVal, SequenceNumber, Signature, SignatureHint,
    SorobanAuthorizationEntry, SorobanTransactionData, TimeBounds, TimePoint, Transaction,
    TransactionEnvelope, TransactionExt, TransactionV1Envelope, UInt256Parts, Uint256, VecM,
};

use crate::domain::ContractId as ContractStrkey;

/// Inclusion fee bid per transaction, in stroops (testnet base fee is 100).
pub const INCLUSION_FEE: u32 = 10_000;

/// Refundable rent margins, from SPP's SDK (measured on testnet 2026-10-09):
/// an entry that crosses an hourly extension step between simulation and
/// apply owes rent the simulation did not include. Unused fee is refunded.
const CODE_RENT_MARGIN: i64 = 1_500_000;
const DATA_RENT_MARGIN: i64 = 25_000;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TxError {
    #[error("invalid XDR: {0}")]
    Xdr(String),
    #[error("fee overflow")]
    FeeOverflow,
    #[error("expected a single InvokeHostFunction operation")]
    UnexpectedShape,
}

impl From<stellar_xdr::Error> for TxError {
    fn from(err: stellar_xdr::Error) -> Self {
        Self::Xdr(err.to_string())
    }
}

/// What the RPC simulation returns that the transaction needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulation {
    pub min_resource_fee: i64,
    pub transaction_data: SorobanTransactionData,
    pub auth: Vec<SorobanAuthorizationEntry>,
}

/// `SHA-256(passphrase)`: the network id every signature commits to.
#[must_use]
pub fn network_id(passphrase: &str) -> [u8; 32] {
    Sha256::digest(passphrase.as_bytes()).into()
}

/// A big-endian 256-bit value as a Soroban `U256`.
#[must_use]
pub fn u256(bytes: [u8; 32]) -> ScVal {
    let word = |i: usize| {
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
        u64::from_be_bytes(chunk)
    };
    ScVal::U256(UInt256Parts {
        hi_hi: word(0),
        hi_lo: word(1),
        lo_hi: word(2),
        lo_lo: word(3),
    })
}

/// Unsigned, unsimulated invocation of `function(args)` on `contract`,
/// valid until `max_time` (unix seconds).
pub fn build_invoke(
    source: &[u8; 32],
    sequence: i64,
    contract: &ContractStrkey,
    function: &str,
    args: Vec<ScVal>,
    max_time: u64,
) -> Result<TransactionEnvelope, TxError> {
    let contract_bytes = stellar_strkey::Contract::from_string(contract.as_str())
        .map_err(|e| TxError::Xdr(e.to_string()))?
        .0;
    let invoke = InvokeHostFunctionOp {
        host_function: HostFunction::InvokeContract(InvokeContractArgs {
            contract_address: ScAddress::Contract(ContractId(Hash(contract_bytes))),
            function_name: ScSymbol::try_from(function)
                .map_err(|()| TxError::Xdr(format!("invalid function name `{function}`")))?,
            args: VecM::try_from(args)?,
        }),
        auth: VecM::default(),
    };
    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(*source)),
        fee: INCLUSION_FEE,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::Time(TimeBounds {
            min_time: TimePoint(0),
            max_time: TimePoint(max_time),
        }),
        memo: Memo::None,
        operations: VecM::try_from(vec![Operation {
            source_account: None,
            body: OperationBody::InvokeHostFunction(invoke),
        }])?,
        ext: TransactionExt::V0,
    };
    Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: VecM::default(),
    }))
}

/// Merges the simulation into the transaction: resource data, auth entries
/// and fee (inclusion + resource fee + rent margin).
pub fn assemble(
    envelope: &TransactionEnvelope,
    sim: &Simulation,
) -> Result<TransactionEnvelope, TxError> {
    let TransactionEnvelope::Tx(v1) = envelope else {
        return Err(TxError::UnexpectedShape);
    };
    let mut tx = v1.tx.clone();
    let [operation] = tx.operations.as_slice() else {
        return Err(TxError::UnexpectedShape);
    };
    let OperationBody::InvokeHostFunction(invoke) = &operation.body else {
        return Err(TxError::UnexpectedShape);
    };

    let margin = rent_margin(&sim.transaction_data.resources.footprint);
    let mut data = sim.transaction_data.clone();
    data.resource_fee = data
        .resource_fee
        .checked_add(margin)
        .ok_or(TxError::FeeOverflow)?;
    let resource_fee = sim
        .min_resource_fee
        .checked_add(margin)
        .ok_or(TxError::FeeOverflow)?;
    tx.fee = i64::from(INCLUSION_FEE)
        .checked_add(resource_fee)
        .and_then(|fee| u32::try_from(fee).ok())
        .ok_or(TxError::FeeOverflow)?;
    tx.ext = TransactionExt::V1(data);

    let mut invoke = invoke.clone();
    invoke.auth = VecM::try_from(sim.auth.clone())?;
    tx.operations = VecM::try_from(vec![Operation {
        source_account: operation.source_account.clone(),
        body: OperationBody::InvokeHostFunction(invoke),
    }])?;

    Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: VecM::default(),
    }))
}

/// Signs the transaction and returns it with its hash.
pub fn sign(
    envelope: TransactionEnvelope,
    key: &SigningKey,
    passphrase: &str,
) -> Result<(TransactionEnvelope, [u8; 32]), TxError> {
    let hash = envelope.hash(network_id(passphrase))?;
    let TransactionEnvelope::Tx(mut v1) = envelope else {
        return Err(TxError::UnexpectedShape);
    };
    let public = key.verifying_key().to_bytes();
    let hint = SignatureHint([public[28], public[29], public[30], public[31]]);
    let signature = Signature(key.sign(&hash).to_bytes().to_vec().try_into()?);
    v1.signatures = VecM::try_from(vec![DecoratedSignature { hint, signature }])?;
    Ok((TransactionEnvelope::Tx(v1), hash))
}

fn rent_margin(footprint: &LedgerFootprint) -> i64 {
    footprint
        .read_only
        .iter()
        .chain(footprint.read_write.iter())
        .map(|key| match key {
            LedgerKey::ContractCode(_) => CODE_RENT_MARGIN,
            LedgerKey::ContractData(data)
                if data.durability == ContractDataDurability::Persistent =>
            {
                DATA_RENT_MARGIN
            }
            _ => 0,
        })
        .fold(0, i64::saturating_add)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Verifier, VerifyingKey};
    use stellar_xdr::{
        ExtensionPoint, LedgerKeyContractCode, LedgerKeyContractData, Limits, ReadXdr,
        SorobanResources, SorobanTransactionDataExt, WriteXdr,
    };

    use super::*;

    const PASSPHRASE: &str = "Test SDF Network ; September 2015";
    const ASP: &str = "CDUJHXHC2DXXOXKBIK5E5QDKD6C3WUSMDIUFF6QTCECBNI4S2656VYD6";

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn contract() -> Result<ContractStrkey, Box<dyn std::error::Error>> {
        Ok(ContractStrkey::parse(ASP)?)
    }

    fn leaf() -> Result<crate::domain::PoolField, Box<dyn std::error::Error>> {
        Ok(crate::domain::PoolField::parse(
            "0x0102030405060708091011121314151617181920212223242526272829303132",
        )?)
    }

    fn simulation(footprint: LedgerFootprint) -> Simulation {
        Simulation {
            min_resource_fee: 50_000,
            transaction_data: SorobanTransactionData {
                ext: SorobanTransactionDataExt::V0,
                resources: SorobanResources {
                    footprint,
                    instructions: 1,
                    disk_read_bytes: 2,
                    write_bytes: 3,
                },
                resource_fee: 50_000,
            },
            auth: Vec::new(),
        }
    }

    fn invoke() -> Result<TransactionEnvelope, Box<dyn std::error::Error>> {
        Ok(build_invoke(
            &key().verifying_key().to_bytes(),
            42,
            &contract()?,
            "insert_leaf",
            vec![u256(leaf()?.field_be())],
            1_800_000_000,
        )?)
    }

    #[test]
    fn network_id_is_the_passphrase_hash() {
        // Well-known testnet network id.
        let id = network_id(PASSPHRASE);
        assert_eq!(
            crate::domain::TxHash::from_bytes(id).as_str(),
            "cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472"
        );
    }

    #[test]
    fn u256_splits_big_endian_words() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            u256(leaf()?.field_be()),
            ScVal::U256(UInt256Parts {
                hi_hi: 0x0102_0304_0506_0708,
                hi_lo: 0x0910_1112_1314_1516,
                lo_hi: 0x1718_1920_2122_2324,
                lo_lo: 0x2526_2728_2930_3132,
            })
        );
        Ok(())
    }

    #[test]
    fn builds_a_time_bounded_invocation() -> Result<(), Box<dyn std::error::Error>> {
        let TransactionEnvelope::Tx(v1) = invoke()? else {
            return Err("expected a v1 envelope".into());
        };
        assert_eq!(v1.tx.seq_num, SequenceNumber(42));
        assert_eq!(
            v1.tx.cond,
            Preconditions::Time(TimeBounds {
                min_time: TimePoint(0),
                max_time: TimePoint(1_800_000_000)
            })
        );
        let [op] = v1.tx.operations.as_slice() else {
            return Err("expected one operation".into());
        };
        let OperationBody::InvokeHostFunction(invoke) = &op.body else {
            return Err("expected an invocation".into());
        };
        let HostFunction::InvokeContract(args) = &invoke.host_function else {
            return Err("expected a contract call".into());
        };
        assert_eq!(args.function_name.to_utf8_string()?, "insert_leaf");
        assert_eq!(args.args.len(), 1);
        Ok(())
    }

    #[test]
    fn assembly_adds_resource_fee_rent_margin_and_auth() -> Result<(), Box<dyn std::error::Error>> {
        let footprint = LedgerFootprint {
            read_only: VecM::try_from(vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                hash: Hash([1; 32]),
            })])?,
            read_write: VecM::try_from(vec![LedgerKey::ContractData(LedgerKeyContractData {
                contract: ScAddress::Contract(ContractId(Hash([2; 32]))),
                key: ScVal::Void,
                durability: ContractDataDurability::Persistent,
            })])?,
        };
        let assembled = assemble(&invoke()?, &simulation(footprint))?;
        let TransactionEnvelope::Tx(v1) = assembled else {
            return Err("expected a v1 envelope".into());
        };

        let margin = CODE_RENT_MARGIN + DATA_RENT_MARGIN;
        assert_eq!(
            i64::from(v1.tx.fee),
            i64::from(INCLUSION_FEE) + 50_000 + margin
        );
        let TransactionExt::V1(data) = &v1.tx.ext else {
            return Err("expected soroban data".into());
        };
        assert_eq!(data.resource_fee, 50_000 + margin);
        Ok(())
    }

    #[test]
    fn assembly_rejects_other_shapes() -> Result<(), Box<dyn std::error::Error>> {
        let TransactionEnvelope::Tx(mut v1) = invoke()? else {
            return Err("expected a v1 envelope".into());
        };
        v1.tx.operations = VecM::default();
        let sim = simulation(LedgerFootprint {
            read_only: VecM::default(),
            read_write: VecM::default(),
        });
        assert_eq!(
            assemble(&TransactionEnvelope::Tx(v1), &sim),
            Err(TxError::UnexpectedShape)
        );
        Ok(())
    }

    #[test]
    fn signature_verifies_against_the_network_hash() -> Result<(), Box<dyn std::error::Error>> {
        let (signed, hash) = sign(invoke()?, &key(), PASSPHRASE)?;
        let TransactionEnvelope::Tx(v1) = &signed else {
            return Err("expected a v1 envelope".into());
        };
        let [decorated] = v1.signatures.as_slice() else {
            return Err("expected one signature".into());
        };
        let verifying: VerifyingKey = key().verifying_key();
        let signature = ed25519_dalek::Signature::from_slice(decorated.signature.0.as_slice())?;
        verifying.verify(&hash, &signature)?;
        assert_eq!(decorated.hint.0, verifying.to_bytes()[28..32]);

        // The hash does not depend on the signatures, and survives a round trip.
        assert_eq!(signed.hash(network_id(PASSPHRASE))?, hash);
        let b64 = signed.to_xdr_base64(Limits::none())?;
        assert_eq!(
            TransactionEnvelope::from_xdr_base64(&b64, Limits::none())?,
            signed
        );
        Ok(())
    }

    #[test]
    fn signing_for_another_network_changes_the_hash() -> Result<(), Box<dyn std::error::Error>> {
        let (_, testnet) = sign(invoke()?, &key(), PASSPHRASE)?;
        let (_, other) = sign(
            invoke()?,
            &key(),
            "Public Global Stellar Network ; September 2015",
        )?;
        assert_ne!(testnet, other);
        let _ = ExtensionPoint::V0;
        Ok(())
    }
}
