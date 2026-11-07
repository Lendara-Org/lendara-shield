//! Minimal Stellar RPC (JSON-RPC 2.0) client: only the methods Shield
//! needs. Spec: <https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods>.

use std::time::Duration;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use stellar_xdr::{
    AccountId, ContractDataDurability, ContractId, Hash, LedgerEntryData, LedgerKey,
    LedgerKeyAccount, LedgerKeyContractData, Limits, PublicKey, ReadXdr, ScAddress, ScSymbol,
    ScVal, ScVec, SorobanAuthorizationEntry, SorobanTransactionData, TransactionEnvelope, Uint256,
    WriteXdr,
};

use super::tx::Simulation;

/// Timeout of every RPC call.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RpcError {
    /// Network, timeout or non-200: the RPC may answer later.
    #[error("RPC unreachable: {0}")]
    Transport(String),
    /// JSON-RPC error object.
    #[error("RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    /// A bootnode's `getEvents` reached the RPC's retention window: continue
    /// on the main RPC from `from_ledger` (SPP's `docs/src/bootnode.md`).
    #[error("bootnode handoff: continue on the RPC from ledger {from_ledger}")]
    Handoff { from_ledger: u32 },
    /// The response did not have the documented shape.
    #[error("unexpected RPC response: {0}")]
    Decode(String),
    #[error("account not found on the network")]
    AccountNotFound,
}

/// Outcome of `sendTransaction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendStatus {
    /// Accepted into the queue.
    Pending,
    /// Already received earlier: same transaction.
    Duplicate,
    /// Queue full; nothing was accepted.
    TryAgainLater,
    /// Rejected before inclusion (e.g. bad sequence, bad auth).
    Error(String),
}

/// Outcome of `getTransaction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetStatus {
    Success,
    Failed(String),
    NotFound,
}

/// One event emitted by a contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractEvent {
    pub ledger: u32,
    pub topic: Vec<ScVal>,
    pub value: ScVal,
}

/// One page of `getEvents`, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventsPage {
    pub events: Vec<ContractEvent>,
    /// Where the next page starts.
    pub cursor: Option<String>,
}

/// Outcome of `simulateTransaction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulationOutcome {
    Ok(Simulation),
    /// The invocation itself fails (contract error, missing auth, ...).
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct SorobanRpc {
    url: String,
    http: reqwest::Client,
}

impl SorobanRpc {
    pub fn new(url: impl Into<String>) -> Result<Self, RpcError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| RpcError::Transport(e.to_string()))?;
        Ok(Self {
            url: url.into(),
            http,
        })
    }

    /// Current sequence number of an account.
    pub async fn account_sequence(&self, account: &[u8; 32]) -> Result<i64, RpcError> {
        let key = LedgerKey::Account(LedgerKeyAccount {
            account_id: AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(*account))),
        });
        let result: LedgerEntries = self
            .call("getLedgerEntries", json!({ "keys": [encode(&key)?] }))
            .await?;
        let entry = result
            .entries
            .unwrap_or_default()
            .into_iter()
            .next()
            .ok_or(RpcError::AccountNotFound)?;
        match decode::<LedgerEntryData>(&entry.xdr)? {
            LedgerEntryData::Account(account) => Ok(account.seq_num.0),
            _ => Err(RpcError::Decode("expected an account ledger entry".into())),
        }
    }

    /// The SPP pool's maximum deposit, in stroops, from the contract's
    /// instance storage (`MaximumDepositAmount`, set at deployment; the pool
    /// has no getter and no setter for it).
    pub async fn pool_max_deposit(&self, pool: &[u8; 32]) -> Result<i64, RpcError> {
        let key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: ScAddress::Contract(ContractId(Hash(*pool))),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
        });
        let result: LedgerEntries = self
            .call("getLedgerEntries", json!({ "keys": [encode(&key)?] }))
            .await?;
        let entry = result
            .entries
            .unwrap_or_default()
            .into_iter()
            .next()
            .ok_or_else(|| RpcError::Decode("the pool contract instance was not found".into()))?;
        max_deposit_from_instance(&decode::<LedgerEntryData>(&entry.xdr)?)
    }

    pub async fn simulate(
        &self,
        envelope: &TransactionEnvelope,
    ) -> Result<SimulationOutcome, RpcError> {
        let raw: RawSimulation = self
            .call(
                "simulateTransaction",
                json!({ "transaction": encode(envelope)? }),
            )
            .await?;
        if let Some(error) = raw.error {
            return Ok(SimulationOutcome::Failed(error));
        }
        let transaction_data = raw
            .transaction_data
            .ok_or_else(|| RpcError::Decode("simulation without transactionData".into()))?;
        let min_resource_fee = raw
            .min_resource_fee
            .ok_or_else(|| RpcError::Decode("simulation without minResourceFee".into()))?
            .parse::<i64>()
            .map_err(|e| RpcError::Decode(format!("minResourceFee: {e}")))?;
        let auth = raw
            .results
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|result| result.auth)
            .unwrap_or_default()
            .iter()
            .map(|entry| decode::<SorobanAuthorizationEntry>(entry))
            .collect::<Result<_, _>>()?;
        Ok(SimulationOutcome::Ok(Simulation {
            min_resource_fee,
            transaction_data: decode::<SorobanTransactionData>(&transaction_data)?,
            auth,
        }))
    }

    pub async fn send(&self, envelope: &TransactionEnvelope) -> Result<SendStatus, RpcError> {
        let raw: RawSend = self
            .call(
                "sendTransaction",
                json!({ "transaction": encode(envelope)? }),
            )
            .await?;
        Ok(match raw.status.as_str() {
            "PENDING" => SendStatus::Pending,
            "DUPLICATE" => SendStatus::Duplicate,
            "TRY_AGAIN_LATER" => SendStatus::TryAgainLater,
            "ERROR" => SendStatus::Error(raw.error_result_xdr.unwrap_or_else(|| "ERROR".into())),
            other => return Err(RpcError::Decode(format!("unknown send status `{other}`"))),
        })
    }

    /// Events of `contract`, from `start_ledger` (first page) or after
    /// `cursor` (next pages). A start ledger older than the RPC's history
    /// window is an [`RpcError::Rpc`].
    pub async fn contract_events(
        &self,
        contract: &str,
        start_ledger: u32,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<EventsPage, RpcError> {
        // Every event of the contract. SPP's bootnode accepts only this
        // filter (`topics: [["**"]]`), and the RPC reads it the same way.
        let filters =
            json!([{ "type": "contract", "topics": [["**"]], "contractIds": [contract] }]);
        let params = match cursor {
            Some(cursor) => json!({
                "filters": filters,
                "pagination": { "cursor": cursor, "limit": limit },
            }),
            None => json!({
                "startLedger": start_ledger,
                "filters": filters,
                "pagination": { "limit": limit },
            }),
        };
        let raw: RawEvents = self.call("getEvents", params).await?;
        let events = raw
            .events
            .into_iter()
            .map(|event| {
                Ok(ContractEvent {
                    ledger: event.ledger,
                    topic: event
                        .topic
                        .iter()
                        .map(|t| decode::<ScVal>(t))
                        .collect::<Result<_, _>>()?,
                    value: decode::<ScVal>(&event.value)?,
                })
            })
            .collect::<Result<_, RpcError>>()?;
        Ok(EventsPage {
            events,
            cursor: raw.cursor.filter(|c| !c.is_empty()),
        })
    }

    pub async fn get_transaction(&self, hash: &str) -> Result<GetStatus, RpcError> {
        let raw: RawGet = self.call("getTransaction", json!({ "hash": hash })).await?;
        Ok(match raw.status.as_str() {
            "SUCCESS" => GetStatus::Success,
            "FAILED" => GetStatus::Failed(raw.result_xdr.unwrap_or_else(|| "FAILED".into())),
            "NOT_FOUND" => GetStatus::NotFound,
            other => {
                return Err(RpcError::Decode(format!(
                    "unknown transaction status `{other}`"
                )));
            }
        })
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T, RpcError> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let response = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| RpcError::Transport(e.to_string()))?;
        if !response.status().is_success() {
            return Err(RpcError::Transport(format!("HTTP {}", response.status())));
        }
        let envelope: JsonRpcResponse<T> = response
            .json()
            .await
            .map_err(|e| RpcError::Decode(e.to_string()))?;
        match (envelope.result, envelope.error) {
            (_, Some(error)) => Err(error.into()),
            (Some(result), None) => Ok(result),
            (None, None) => Err(RpcError::Decode("neither result nor error".into())),
        }
    }
}

fn encode<T: WriteXdr>(value: &T) -> Result<String, RpcError> {
    value
        .to_xdr_base64(Limits::none())
        .map_err(|e| RpcError::Decode(e.to_string()))
}

/// Reads `MaximumDepositAmount` (a `u256`) from a contract instance entry.
fn max_deposit_from_instance(entry: &LedgerEntryData) -> Result<i64, RpcError> {
    let LedgerEntryData::ContractData(data) = entry else {
        return Err(RpcError::Decode("expected a contract data entry".into()));
    };
    let ScVal::ContractInstance(instance) = &data.val else {
        return Err(RpcError::Decode("expected a contract instance".into()));
    };
    let wanted = ScVal::Vec(Some(ScVec(
        vec![ScVal::Symbol(
            ScSymbol::try_from("MaximumDepositAmount")
                .map_err(|()| RpcError::Decode("symbol".into()))?,
        )]
        .try_into()
        .map_err(|_| RpcError::Decode("key".into()))?,
    )));
    let value = instance
        .storage
        .as_ref()
        .and_then(|map| map.iter().find(|entry| entry.key == wanted))
        .map(|entry| &entry.val)
        .ok_or_else(|| RpcError::Decode("the pool has no MaximumDepositAmount".into()))?;
    let ScVal::U256(parts) = value else {
        return Err(RpcError::Decode(
            "MaximumDepositAmount is not a u256".into(),
        ));
    };
    if parts.hi_hi != 0 || parts.hi_lo != 0 || parts.lo_hi != 0 {
        return Err(RpcError::Decode(
            "MaximumDepositAmount does not fit in i64".into(),
        ));
    }
    i64::try_from(parts.lo_lo)
        .map_err(|_| RpcError::Decode("MaximumDepositAmount does not fit in i64".into()))
}

fn decode<T: ReadXdr>(b64: &str) -> Result<T, RpcError> {
    T::from_xdr_base64(b64, Limits::none()).map_err(|e| RpcError::Decode(e.to_string()))
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

/// JSON-RPC error code of a bootnode's retention handoff.
const HANDOFF_CODE: i64 = -32002;

impl From<JsonRpcError> for RpcError {
    fn from(error: JsonRpcError) -> Self {
        let from_ledger = error
            .data
            .as_ref()
            .and_then(|data| data.get("fromLedger"))
            .and_then(Value::as_u64)
            .and_then(|ledger| u32::try_from(ledger).ok());
        match (error.code, from_ledger) {
            (HANDOFF_CODE, Some(from_ledger)) => Self::Handoff { from_ledger },
            (code, _) => Self::Rpc {
                code,
                message: error.message,
            },
        }
    }
}

#[derive(Deserialize)]
struct RawEvents {
    #[serde(default)]
    events: Vec<RawEvent>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct RawEvent {
    ledger: u32,
    #[serde(default)]
    topic: Vec<String>,
    value: String,
}

#[derive(Deserialize)]
struct LedgerEntries {
    entries: Option<Vec<LedgerEntry>>,
}

#[derive(Deserialize)]
struct LedgerEntry {
    xdr: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSimulation {
    transaction_data: Option<String>,
    min_resource_fee: Option<String>,
    results: Option<Vec<RawHostFunctionResult>>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct RawHostFunctionResult {
    #[serde(default)]
    auth: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSend {
    status: String,
    error_result_xdr: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGet {
    status: String,
    result_xdr: Option<String>,
}

#[cfg(test)]
mod tests {
    use stellar_xdr::{
        ContractDataEntry, ContractExecutable, ExtensionPoint, ScContractInstance, ScMap,
        ScMapEntry, UInt256Parts,
    };

    use super::*;

    fn instance(entries: Vec<(&str, ScVal)>) -> LedgerEntryData {
        let storage = entries
            .into_iter()
            .map(|(name, val)| ScMapEntry {
                key: ScVal::Vec(Some(ScVec(
                    vec![ScVal::Symbol(ScSymbol::try_from(name).unwrap_or_default())]
                        .try_into()
                        .unwrap_or_default(),
                ))),
                val,
            })
            .collect::<Vec<_>>();
        LedgerEntryData::ContractData(ContractDataEntry {
            ext: ExtensionPoint::V0,
            contract: ScAddress::Contract(ContractId(Hash([7; 32]))),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
            val: ScVal::ContractInstance(ScContractInstance {
                executable: ContractExecutable::Wasm(Hash([1; 32])),
                storage: Some(ScMap(storage.try_into().unwrap_or_default())),
            }),
        })
    }

    fn u256(lo_lo: u64) -> ScVal {
        ScVal::U256(UInt256Parts {
            hi_hi: 0,
            hi_lo: 0,
            lo_hi: 0,
            lo_lo,
        })
    }

    #[test]
    fn reads_the_pools_maximum_deposit_from_its_instance_storage() {
        // As deployed on testnet: 1,000 USDC at 7 decimals.
        let entry = instance(vec![
            ("Admin", ScVal::Void),
            ("MaximumDepositAmount", u256(10_000_000_000)),
        ]);
        assert_eq!(max_deposit_from_instance(&entry), Ok(10_000_000_000));
    }

    #[test]
    fn refuses_a_missing_or_malformed_maximum_deposit() {
        assert!(max_deposit_from_instance(&instance(vec![("Admin", ScVal::Void)])).is_err());
        assert!(
            max_deposit_from_instance(&instance(vec![("MaximumDepositAmount", ScVal::U32(5))]))
                .is_err()
        );
        let huge = ScVal::U256(UInt256Parts {
            hi_hi: 0,
            hi_lo: 0,
            lo_hi: 1,
            lo_lo: 0,
        });
        assert!(
            max_deposit_from_instance(&instance(vec![("MaximumDepositAmount", huge)])).is_err()
        );
        assert!(
            max_deposit_from_instance(&instance(vec![("MaximumDepositAmount", u256(u64::MAX))]))
                .is_err()
        );
        let account = LedgerEntryData::Account(stellar_xdr::AccountEntry::default());
        assert!(max_deposit_from_instance(&account).is_err());
    }
}
