//! [`AnchorClient`] over SEP-1 → SEP-10 → SEP-24.
//!
//! 1. SEP-1: read `SIGNING_KEY`, `WEB_AUTH_ENDPOINT`, `TRANSFER_SERVER_SEP0024`
//!    from `https://<home_domain>/.well-known/stellar.toml` (cached).
//! 2. SEP-10: authenticate the intake account with the user's customer memo,
//!    verifying the challenge before signing it.
//! 3. SEP-24: start an interactive deposit to the intake with the intent's
//!    memo (`memo_type: id`).

use std::{fmt, sync::Arc, time::Duration};

use async_trait::async_trait;
use ed25519_dalek::SigningKey;
use reqwest::{StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{Value, json};
use stellar_xdr::{Limits, ReadXdr, TransactionEnvelope, WriteXdr};
use tokio::sync::OnceCell;

use super::{
    sep1::{self, StellarToml},
    sep10::{self, Expectations},
};
use crate::{
    domain::AccountId,
    ports::{AnchorClient, AnchorDeposit, AnchorError, AnchorStatus, Clock, StartDeposit},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Sep24AnchorConfig {
    pub home_domain: String,
    /// Intake account seed: authenticates with SEP-10.
    pub intake_secret: SecretString,
    /// Issuer of the asset the anchor pays out.
    pub asset_issuer: AccountId,
    pub network_passphrase: String,
}

impl fmt::Debug for Sep24AnchorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sep24AnchorConfig")
            .field("home_domain", &self.home_domain)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Sep24ConfigError {
    #[error("SHIELD_INTAKE_SECRET is not a valid S... secret seed")]
    InvalidSecret,
    #[error("HTTP client: {0}")]
    Http(String),
}

/// The anchor endpoints, validated.
#[derive(Debug, Clone)]
struct Endpoints {
    server_key: [u8; 32],
    web_auth: Url,
    web_auth_domain: String,
    transfer_server: Url,
}

pub struct Sep24AnchorClient {
    http: reqwest::Client,
    home_domain: String,
    intake: SigningKey,
    intake_address: String,
    asset_issuer: AccountId,
    passphrase: String,
    clock: Arc<dyn Clock>,
    endpoints: OnceCell<Endpoints>,
}

impl fmt::Debug for Sep24AnchorClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sep24AnchorClient")
            .field("home_domain", &self.home_domain)
            .field("intake", &self.intake_address)
            .finish_non_exhaustive()
    }
}

impl Sep24AnchorClient {
    pub fn new(config: Sep24AnchorConfig, clock: Arc<dyn Clock>) -> Result<Self, Sep24ConfigError> {
        let seed =
            stellar_strkey::ed25519::PrivateKey::from_string(config.intake_secret.expose_secret())
                .map_err(|_| Sep24ConfigError::InvalidSecret)?;
        let intake = SigningKey::from_bytes(&seed.0);
        let intake_address = stellar_strkey::ed25519::PublicKey(intake.verifying_key().to_bytes())
            .to_string()
            .to_string();
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|e| Sep24ConfigError::Http(e.to_string()))?,
            home_domain: config.home_domain,
            intake,
            intake_address,
            asset_issuer: config.asset_issuer,
            passphrase: config.network_passphrase,
            clock,
            endpoints: OnceCell::new(),
        })
    }

    async fn endpoints(&self) -> Result<&Endpoints, AnchorError> {
        self.endpoints.get_or_try_init(|| self.discover()).await
    }

    /// SEP-1: fetch and validate the anchor's `stellar.toml`.
    async fn discover(&self) -> Result<Endpoints, AnchorError> {
        let url = base_url(&self.home_domain)?
            .join("/.well-known/stellar.toml")
            .map_err(|e| refused(e.to_string()))?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| unavailable(&e))?;
        let body = ok_body(response).await?;
        let toml: StellarToml = sep1::parse(&body).map_err(|e| refused(e.to_string()))?;
        if toml.network_passphrase != self.passphrase {
            return Err(refused("the anchor is not on the expected network".into()));
        }
        let server_key = stellar_strkey::ed25519::PublicKey::from_string(&toml.signing_key)
            .map_err(|_| refused("SIGNING_KEY is not a G... account".into()))?
            .0;
        let web_auth = secure_url(&toml.web_auth_endpoint)?;
        let web_auth_domain = match (web_auth.host_str(), web_auth.port()) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            (Some(host), None) => host.to_owned(),
            (None, _) => return Err(refused("WEB_AUTH_ENDPOINT has no host".into())),
        };
        Ok(Endpoints {
            server_key,
            web_auth,
            web_auth_domain,
            transfer_server: secure_url(&toml.transfer_server_sep0024)?,
        })
    }

    /// SEP-10: returns a JWT for the intake account as `customer_memo`.
    async fn authenticate(
        &self,
        endpoints: &Endpoints,
        customer_memo: u64,
    ) -> Result<SecretString, AnchorError> {
        let mut url = endpoints.web_auth.clone();
        url.query_pairs_mut()
            .append_pair("account", &self.intake_address)
            .append_pair("memo", &customer_memo.to_string())
            .append_pair("home_domain", &self.home_domain);
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| unavailable(&e))?;
        let challenge: ChallengeResponse = ok_json(response).await?;
        if challenge
            .network_passphrase
            .as_deref()
            .is_some_and(|p| p != self.passphrase)
        {
            return Err(refused("challenge for another network".into()));
        }

        let envelope = TransactionEnvelope::from_xdr_base64(&challenge.transaction, Limits::none())
            .map_err(|e| refused(format!("challenge is not valid XDR: {e}")))?;
        let now = u64::try_from(self.clock.now().timestamp()).unwrap_or_default();
        sep10::verify(
            &envelope,
            &Expectations {
                server_key: endpoints.server_key,
                client: self.intake.verifying_key().to_bytes(),
                home_domain: &self.home_domain,
                web_auth_domain: &endpoints.web_auth_domain,
                memo: Some(customer_memo),
                passphrase: &self.passphrase,
                now,
            },
        )
        .map_err(|e| refused(format!("SEP-10 challenge refused: {e}")))?;
        let signed = sep10::sign(envelope, &self.intake, &self.passphrase)
            .map_err(|e| refused(e.to_string()))?
            .to_xdr_base64(Limits::none())
            .map_err(|e| refused(e.to_string()))?;

        let response = self
            .http
            .post(endpoints.web_auth.clone())
            .json(&json!({ "transaction": signed }))
            .send()
            .await
            .map_err(|e| unavailable(&e))?;
        let token: TokenResponse = ok_json(response).await?;
        Ok(SecretString::from(token.token))
    }
}

#[async_trait]
impl AnchorClient for Sep24AnchorClient {
    fn home_domain(&self) -> &str {
        &self.home_domain
    }

    async fn start_deposit(&self, request: &StartDeposit) -> Result<AnchorDeposit, AnchorError> {
        let endpoints = self.endpoints().await?;
        let customer_memo = u64::try_from(request.customer_memo)
            .map_err(|_| refused("customer memo must be positive".into()))?;
        let deposit_memo = u64::try_from(request.deposit_memo)
            .map_err(|_| refused("deposit memo must be positive".into()))?;
        let token = self.authenticate(endpoints, customer_memo).await?;

        let url = join(
            &endpoints.transfer_server,
            "transactions/deposit/interactive",
        )?;
        let response = self
            .http
            .post(url)
            .bearer_auth(token.expose_secret())
            .json(&json!({
                "asset_code": request.asset_code,
                "asset_issuer": self.asset_issuer.as_str(),
                "account": self.intake_address,
                "memo": deposit_memo.to_string(),
                "memo_type": "id",
                "source_asset": request.source_asset,
            }))
            .send()
            .await
            .map_err(|e| unavailable(&e))?;
        let deposit: InteractiveResponse = ok_json(response).await?;
        if deposit.kind != "interactive_customer_info_needed" {
            return Err(refused(format!(
                "unexpected SEP-24 response type `{}`",
                deposit.kind
            )));
        }
        secure_url(&deposit.url)?;
        Ok(AnchorDeposit {
            id: deposit.id,
            interactive_url: deposit.url,
        })
    }

    async fn deposit_status(
        &self,
        customer_memo: i64,
        anchor_tx_id: &str,
    ) -> Result<AnchorStatus, AnchorError> {
        let endpoints = self.endpoints().await?;
        let customer_memo = u64::try_from(customer_memo)
            .map_err(|_| refused("customer memo must be positive".into()))?;
        let token = self.authenticate(endpoints, customer_memo).await?;
        let mut url = join(&endpoints.transfer_server, "transaction")?;
        url.query_pairs_mut().append_pair("id", anchor_tx_id);
        let response = self
            .http
            .get(url)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|e| unavailable(&e))?;
        let body: TransactionResponse = ok_json(response).await?;
        Ok(map_status(
            &body.transaction.status,
            body.transaction.message,
        ))
    }
}

/// SEP-24 transaction statuses, reduced to what Shield acts on.
fn map_status(status: &str, message: Option<String>) -> AnchorStatus {
    match status {
        "completed" => AnchorStatus::Completed,
        "expired" => AnchorStatus::Expired,
        "error" | "refunded" | "no_market" | "too_small" | "too_large" => {
            AnchorStatus::Failed(message.unwrap_or_else(|| status.to_owned()))
        }
        // `incomplete`, `pending_*` and any status added later: keep waiting.
        _ => AnchorStatus::Pending,
    }
}

#[derive(Deserialize)]
struct ChallengeResponse {
    transaction: String,
    network_passphrase: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

#[derive(Deserialize)]
struct TransactionResponse {
    transaction: TransactionStatus,
}

#[derive(Deserialize)]
struct TransactionStatus {
    status: String,
    message: Option<String>,
}

#[derive(Deserialize)]
struct InteractiveResponse {
    #[serde(rename = "type")]
    kind: String,
    url: String,
    id: String,
}

/// `https://<home_domain>`, or `http://` for a local anchor (tests, dev).
fn base_url(home_domain: &str) -> Result<Url, AnchorError> {
    let scheme = if is_local(home_domain) {
        "http"
    } else {
        "https"
    };
    Url::parse(&format!("{scheme}://{home_domain}")).map_err(|e| refused(e.to_string()))
}

/// Anchor URLs must be HTTPS, except on localhost.
fn secure_url(raw: &str) -> Result<Url, AnchorError> {
    let url = Url::parse(raw).map_err(|e| refused(format!("invalid URL `{raw}`: {e}")))?;
    let local = url.host_str().is_some_and(is_local);
    if url.scheme() == "https" || (url.scheme() == "http" && local) {
        Ok(url)
    } else {
        Err(refused(format!("`{raw}` must use https")))
    }
}

fn is_local(host: &str) -> bool {
    let host = host.split(':').next().unwrap_or(host);
    host == "localhost" || host == "127.0.0.1"
}

/// `base` + `path`, keeping `base`'s own path (`.../sep24` + `transactions/...`).
fn join(base: &Url, path: &str) -> Result<Url, AnchorError> {
    let mut base = base.clone();
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    base.join(path).map_err(|e| refused(e.to_string()))
}

async fn ok_body(response: reqwest::Response) -> Result<String, AnchorError> {
    let status = response.status();
    let body = response.text().await.map_err(|e| unavailable(&e))?;
    check_status(status, &body)?;
    Ok(body)
}

async fn ok_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, AnchorError> {
    let body = ok_body(response).await?;
    serde_json::from_str(&body)
        .map_err(|e| unavailable_msg(format!("unexpected anchor response: {e}")))
}

/// 4xx: the anchor refused (do not retry blindly). 5xx: temporary.
fn check_status(status: StatusCode, body: &str) -> Result<(), AnchorError> {
    if status.is_success() {
        return Ok(());
    }
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| status.to_string());
    if status.is_client_error() {
        Err(refused(detail))
    } else {
        Err(unavailable_msg(detail))
    }
}

fn refused(reason: String) -> AnchorError {
    AnchorError::Refused(reason)
}

fn unavailable(err: &reqwest::Error) -> AnchorError {
    AnchorError::Unavailable(err.to_string())
}

fn unavailable_msg(reason: String) -> AnchorError {
    AnchorError::Unavailable(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_anchors_may_use_http() {
        assert!(secure_url("https://anchor.example/auth").is_ok());
        assert!(secure_url("http://127.0.0.1:8000/auth").is_ok());
        assert!(secure_url("http://localhost/auth").is_ok());
        assert!(matches!(
            secure_url("http://anchor.example/auth"),
            Err(AnchorError::Refused(_))
        ));
        assert!(matches!(
            secure_url("ftp://anchor.example"),
            Err(AnchorError::Refused(_))
        ));
        assert!(matches!(
            secure_url("not a url"),
            Err(AnchorError::Refused(_))
        ));
    }

    #[test]
    fn joins_paths_under_the_transfer_server() -> Result<(), AnchorError> {
        let base = secure_url("https://anchor.example/sep24")?;
        assert_eq!(
            join(&base, "transactions/deposit/interactive")?.as_str(),
            "https://anchor.example/sep24/transactions/deposit/interactive"
        );
        let slash = secure_url("https://anchor.example/sep24/")?;
        assert_eq!(
            join(&slash, "info")?.as_str(),
            "https://anchor.example/sep24/info"
        );
        Ok(())
    }

    #[test]
    fn maps_sep24_statuses() {
        for pending in [
            "incomplete",
            "pending_user_transfer_start",
            "pending_anchor",
            "pending_stellar",
            "something_new",
        ] {
            assert_eq!(
                map_status(pending, None),
                AnchorStatus::Pending,
                "{pending}"
            );
        }
        assert_eq!(map_status("completed", None), AnchorStatus::Completed);
        assert_eq!(map_status("expired", None), AnchorStatus::Expired);
        assert_eq!(
            map_status("error", Some("KYC rejected (sandbox scenario).".into())),
            AnchorStatus::Failed("KYC rejected (sandbox scenario).".into())
        );
        for failed in ["refunded", "no_market", "too_small", "too_large"] {
            assert_eq!(
                map_status(failed, None),
                AnchorStatus::Failed(failed.into()),
                "{failed}"
            );
        }
    }

    #[test]
    fn classifies_http_errors() {
        assert!(check_status(StatusCode::OK, "").is_ok());
        assert_eq!(
            check_status(
                StatusCode::BAD_REQUEST,
                r#"{"error":"memo of type id must be..."}"#
            ),
            Err(AnchorError::Refused("memo of type id must be...".into()))
        );
        assert!(matches!(
            check_status(StatusCode::BAD_GATEWAY, "oops"),
            Err(AnchorError::Unavailable(_))
        ));
    }
}
