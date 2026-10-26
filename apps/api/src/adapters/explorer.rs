//! [`Explorer`] on Horizon: what any observer can read about a transaction.
//! Docs: <https://developers.stellar.org/docs/data/apis/horizon/api-reference>.
//!
//! A pool call is an `invoke_host_function` whose first parameter (the
//! contract) is Shield's pool; its public amount is the asset moved into
//! or out of the pool contract (`asset_balance_changes`).

use std::{collections::BTreeSet, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde_json::Value;
use stellar_xdr::{Limits, ReadXdr, ScAddress, ScVal};

use crate::{
    domain::{AccountId, ContractId, DecimalAmount, TxHash},
    ports::{Explorer, LedgerError, PublicTx},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// Horizon's largest page; a transaction has at most 100 operations.
const PAGE: &str = "200";

#[derive(Debug, Clone)]
pub struct HorizonExplorer {
    http: reqwest::Client,
    base: Url,
    pool: ContractId,
}

impl HorizonExplorer {
    pub fn new(base_url: &str, pool: ContractId) -> Result<Self, LedgerError> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|e| LedgerError(e.to_string()))?,
            base: Url::parse(base_url).map_err(|e| LedgerError(format!("HORIZON_URL: {e}")))?,
            pool,
        })
    }

    /// `None` on 404.
    async fn get(&self, path: &str) -> Result<Option<Value>, LedgerError> {
        let mut url = self
            .base
            .join(path)
            .map_err(|e| LedgerError(e.to_string()))?;
        if path.ends_with("operations") || path.ends_with("effects") {
            url.query_pairs_mut().append_pair("limit", PAGE);
        }
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| LedgerError(e.to_string()))?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            status if status.is_success() => response
                .json()
                .await
                .map(Some)
                .map_err(|e| LedgerError(format!("unexpected Horizon response: {e}"))),
            status => Err(LedgerError(format!("Horizon answered {status}"))),
        }
    }

    async fn records(&self, path: &str) -> Result<Vec<Value>, LedgerError> {
        let page = self.get(path).await?.unwrap_or(Value::Null);
        Ok(page["_embedded"]["records"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    fn is_pool_call(&self, op: &Value) -> bool {
        op["type"] == "invoke_host_function"
            && contract_of(&op["parameters"][0]).as_deref() == Some(self.pool.as_str())
    }

    /// Net stroops into (+) or out of (−) the pool across `ops`.
    fn pool_ext(&self, ops: &[Value]) -> Result<i128, LedgerError> {
        let mut total = 0i128;
        for change in ops
            .iter()
            .filter(|op| self.is_pool_call(op))
            .filter_map(|op| op["asset_balance_changes"].as_array())
            .flatten()
        {
            let amount = change["amount"].as_str().unwrap_or_default();
            let stroops = i128::from(
                DecimalAmount::parse(amount)
                    .map_err(|e| LedgerError(format!("bad balance change: {e}")))?
                    .to_stroops(),
            );
            if change["to"] == self.pool.as_str() {
                total += stroops;
            }
            if change["from"] == self.pool.as_str() {
                total -= stroops;
            }
        }
        Ok(total)
    }
}

#[async_trait]
impl Explorer for HorizonExplorer {
    async fn transaction(&self, hash: &TxHash) -> Result<Option<PublicTx>, LedgerError> {
        let Some(tx) = self.get(&format!("transactions/{hash}")).await? else {
            return Ok(None);
        };
        let header: TxHeader = serde_json::from_value(tx.clone())
            .map_err(|e| LedgerError(format!("unexpected Horizon transaction: {e}")))?;
        let ops = self
            .records(&format!("transactions/{hash}/operations"))
            .await?;
        let effects = self
            .records(&format!("transactions/{hash}/effects"))
            .await?;

        let is_pool_transaction = ops.iter().any(|op| self.is_pool_call(op));
        let mut visible = BTreeSet::new();
        for value in std::iter::once(&tx).chain(&ops).chain(&effects) {
            collect_accounts(value, &mut visible);
        }
        Ok(Some(PublicTx {
            hash: hash.clone(),
            ledger: header.ledger,
            created_at: header.created_at,
            source_account: AccountId::parse(&header.source_account)
                .map_err(|e| LedgerError(e.to_string()))?,
            is_pool_transaction,
            pool_ext_stroops: if is_pool_transaction {
                Some(self.pool_ext(&ops)?)
            } else {
                None
            },
            visible_accounts: visible
                .iter()
                .filter_map(|raw| AccountId::parse(raw).ok())
                .collect(),
        }))
    }

    async fn pool_transactions_in_ledger(&self, ledger: u32) -> Result<Vec<TxHash>, LedgerError> {
        let ops = self
            .records(&format!("ledgers/{ledger}/operations"))
            .await?;
        let mut hashes: Vec<TxHash> = Vec::new();
        for op in ops.iter().filter(|op| self.is_pool_call(op)) {
            let hash = TxHash::parse(op["transaction_hash"].as_str().unwrap_or_default())
                .map_err(|e| LedgerError(e.to_string()))?;
            if !hashes.contains(&hash) {
                hashes.push(hash);
            }
        }
        Ok(hashes)
    }
}

#[derive(Deserialize)]
struct TxHeader {
    ledger: u32,
    created_at: DateTime<Utc>,
    source_account: String,
}

/// The contract strkey of a Horizon `Address` parameter (`ScVal` XDR).
fn contract_of(param: &Value) -> Option<String> {
    if param["type"] != "Address" {
        return None;
    }
    match ScVal::from_xdr_base64(param["value"].as_str()?, Limits::none()).ok()? {
        ScVal::Address(ScAddress::Contract(id)) => {
            Some(format!("{}", stellar_strkey::Contract(id.0.0)))
        }
        _ => None,
    }
}

/// Every string in `value` that is a Stellar account (`G…`).
fn collect_accounts(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        Value::String(s) if s.len() == 56 && s.starts_with('G') => {
            if AccountId::parse(s).is_ok() {
                into.insert(s.clone());
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_accounts(v, into)),
        Value::Object(map) => map.values().for_each(|v| collect_accounts(v, into)),
        _ => {}
    }
}
