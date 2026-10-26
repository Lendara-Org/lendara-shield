//! Minimal Stellar RPC (JSON-RPC 2.0) client: only the four methods Shield
//! needs. Spec: <https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods>.

use std::time::Duration;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use stellar_xdr::{
    AccountId, LedgerEntryData, LedgerKey, LedgerKeyAccount, Limits, PublicKey, ReadXdr,
    SorobanAuthorizationEntry, SorobanTransactionData, TransactionEnvelope, Uint256, WriteXdr,
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
            (_, Some(error)) => Err(RpcError::Rpc {
                code: error.code,
                message: error.message,
            }),
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
