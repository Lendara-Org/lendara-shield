//! [`IntakeLedger`] on Horizon: `GET /accounts/{intake}/payments` with the
//! transactions joined (for the memo). Docs:
//! <https://developers.stellar.org/docs/data/apis/horizon/api-reference/list-all-payments>.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Url;
use serde::Deserialize;

use crate::{
    domain::{AccountId, DecimalAmount, TxHash},
    ports::{IncomingPayment, IntakeLedger, LedgerError, PaymentPage},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct HorizonLedger {
    http: reqwest::Client,
    base: Url,
    intake: AccountId,
}

impl HorizonLedger {
    pub fn new(base_url: &str, intake: AccountId) -> Result<Self, LedgerError> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|e| LedgerError(e.to_string()))?,
            base: Url::parse(base_url).map_err(|e| LedgerError(format!("HORIZON_URL: {e}")))?,
            intake,
        })
    }
}

#[async_trait]
impl IntakeLedger for HorizonLedger {
    async fn payments_after(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<PaymentPage, LedgerError> {
        let mut url = self
            .base
            .join(&format!("accounts/{}/payments", self.intake))
            .map_err(|e| LedgerError(e.to_string()))?;
        url.query_pairs_mut()
            .append_pair("order", "asc")
            .append_pair("limit", &limit.to_string())
            .append_pair("join", "transactions");
        if let Some(cursor) = cursor {
            url.query_pairs_mut().append_pair("cursor", cursor);
        }
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| LedgerError(e.to_string()))?;
        if !response.status().is_success() {
            return Err(LedgerError(format!(
                "Horizon answered {}",
                response.status()
            )));
        }
        let page: Page = response
            .json()
            .await
            .map_err(|e| LedgerError(format!("unexpected Horizon response: {e}")))?;
        Ok(to_page(page, &self.intake))
    }
}

/// Keeps successful classic payments **to** the intake; the cursor still
/// advances over everything else.
fn to_page(page: Page, intake: &AccountId) -> PaymentPage {
    let records = page.embedded.records;
    let next_cursor = records.last().map(|r| r.paging_token.clone());
    let payments = records
        .into_iter()
        .filter(|r| r.kind == "payment" && r.to.as_deref() == Some(intake.as_str()))
        .filter(|r| r.transaction_successful.unwrap_or(true))
        .filter_map(|r| incoming(&r))
        .collect();
    PaymentPage {
        payments,
        next_cursor,
    }
}

fn incoming(record: &Record) -> Option<IncomingPayment> {
    let parsed = (|| {
        let memo = match record.transaction.as_ref() {
            Some(tx) if tx.memo_type == "id" => Some(tx.memo.as_deref()?.parse::<u64>().ok()?),
            _ => None,
        };
        Some(IncomingPayment {
            tx_hash: TxHash::parse(&record.transaction_hash).ok()?,
            from: AccountId::parse(record.from.as_deref()?).ok()?,
            asset_code: record.asset_code.clone().unwrap_or_else(|| "XLM".into()),
            asset_issuer: record.asset_issuer.clone().unwrap_or_default(),
            amount: DecimalAmount::parse(record.amount.as_deref()?).ok()?,
            memo,
            operation_id: record.id.clone(),
        })
    })();
    if parsed.is_none() {
        tracing::warn!(operation_id = %record.id, "skipping a Horizon payment that does not parse");
    }
    parsed
}

#[derive(Deserialize)]
struct Page {
    #[serde(rename = "_embedded")]
    embedded: Embedded,
}

#[derive(Deserialize)]
struct Embedded {
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Record {
    id: String,
    paging_token: String,
    #[serde(rename = "type")]
    kind: String,
    transaction_hash: String,
    transaction_successful: Option<bool>,
    from: Option<String>,
    to: Option<String>,
    asset_code: Option<String>,
    asset_issuer: Option<String>,
    amount: Option<String>,
    transaction: Option<Transaction>,
}

#[derive(Deserialize)]
struct Transaction {
    memo_type: String,
    memo: Option<String>,
}
