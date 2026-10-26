//! The pool deployment written by `make setup-testnet` (SPP's
//! `deployments.json`), validated against what Shield requires.
//!
//! Only the fields Shield uses are read. Anything that does not match
//! Shield's design (one enabled pool on testnet, a classic asset, an
//! allowlist-blocklist policy with a Global View Key) is rejected at startup.

use std::path::Path;

use serde::Deserialize;

use crate::domain::{AccountId, ContractId};

/// The only network Shield runs on (CLAUDE.md §0.5).
pub const TESTNET: &str = "testnet";

/// Validated deployment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolDeployment {
    pub kdf_domain: String,
    pub asp_membership: ContractId,
    pub asp_non_membership: ContractId,
    pub pool: ContractId,
    pub asset: ClassicAsset,
    pub gvk_mode: GvkMode,
}

/// The pool's asset: a classic Stellar asset wrapped by its SAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicAsset {
    pub code: String,
    pub issuer: AccountId,
    pub contract: ContractId,
}

/// How much the Global View Key can decrypt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GvkMode {
    /// Note outputs only.
    ViewOnly,
    /// Inputs and outputs: full flows.
    Traceable,
}

#[derive(Debug, thiserror::Error)]
pub enum DeploymentError {
    #[error("cannot read deployment file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("deployment file is not a valid SPP manifest: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("deployment is for `{0}`; Shield only runs on testnet")]
    NotTestnet(String),
    #[error("deployment must have exactly one enabled pool, found {0}")]
    PoolCount(usize),
    #[error("the pool's asset must be a classic asset (code + issuer)")]
    NotClassicAsset,
    #[error("the pool must use the allowlist-blocklist ASP policy")]
    WrongPolicy,
    #[error("the pool must have a Global View Key (gvk-viewonly or gvk-traceable)")]
    NoGlobalViewKey,
}

impl PoolDeployment {
    /// Reads and validates the manifest at `path`.
    pub fn load(path: &Path) -> Result<Self, DeploymentError> {
        let raw = std::fs::read_to_string(path).map_err(|source| DeploymentError::Read {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_json(&raw)
    }

    /// Parses and validates a manifest.
    pub fn from_json(raw: &str) -> Result<Self, DeploymentError> {
        let manifest: Manifest = serde_json::from_str(raw)?;
        if manifest.network != TESTNET {
            return Err(DeploymentError::NotTestnet(manifest.network));
        }

        let mut enabled: Vec<ManifestPool> = manifest
            .pools
            .into_iter()
            .filter(|pool| pool.enabled)
            .collect();
        if enabled.len() != 1 {
            return Err(DeploymentError::PoolCount(enabled.len()));
        }
        let pool = enabled.remove(0);

        let ManifestAsset::Classic { code, issuer } = pool.asset else {
            return Err(DeploymentError::NotClassicAsset);
        };
        let mut policy = pool.policy_flags;
        policy.sort_unstable();
        if policy != [AspPolicy::Allowlist, AspPolicy::Blocklist] {
            return Err(DeploymentError::WrongPolicy);
        }
        let gvk_mode = pool.gvk_mode.ok_or(DeploymentError::NoGlobalViewKey)?;

        Ok(Self {
            kdf_domain: manifest.kdf_domain,
            asp_membership: manifest.asp_membership,
            asp_non_membership: manifest.asp_non_membership,
            pool: pool.pool_contract_id,
            asset: ClassicAsset {
                code,
                issuer,
                contract: pool.token_contract_id,
            },
            gvk_mode,
        })
    }
}

/// Subset of SPP's `deployments.json` (field names as SPP writes them).
#[derive(Debug, Deserialize)]
struct Manifest {
    network: String,
    kdf_domain: String,
    asp_membership: ContractId,
    asp_non_membership: ContractId,
    pools: Vec<ManifestPool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestPool {
    pool_contract_id: ContractId,
    token_contract_id: ContractId,
    enabled: bool,
    policy_flags: Vec<AspPolicy>,
    asset: ManifestAsset,
    gvk_mode: Option<GvkMode>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum ManifestAsset {
    Native,
    Classic { code: String, issuer: AccountId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AspPolicy {
    Allowlist,
    Blocklist,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const USDC_ISSUER: &str = "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5";

    fn manifest() -> Value {
        json!({
            "network": "testnet",
            "kdf_domain": "Lendara Shield",
            "asp_membership": "CBXCTA27XVLULCT4O5LCE2ETKW7A6PXEOFXOKHHFP4JCPRT72T7HBQLL",
            "asp_non_membership": "CBGH7D7JZ3FRJ6ILEDRABBUWTEEV42CKBWTPQQWMHNF7H3OXWREEOAFA",
            "public_key_registry": "CDKEJFD2TJGJ5TL2COAA6ULRDCO55EPBCYB2LZD2QKSR4MAO6NPRRMWL",
            "pools": [{
                "poolContractId": "CCAEA4MKORUJV3G3V47SKWLJKQZBYGGFU42JSCXNUIJIZMNN2MSDLFMO",
                "tokenContractId": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
                "deploymentLedger": 5_388_674,
                "enabled": true,
                "policyFlags": ["allowlist", "blocklist"],
                "asset": {"kind": "classic", "code": "USDC", "issuer": USDC_ISSUER},
                "gvkMode": "traceable"
            }]
        })
    }

    fn parse(value: &Value) -> Result<PoolDeployment, DeploymentError> {
        PoolDeployment::from_json(&value.to_string())
    }

    #[test]
    fn accepts_shields_pool() -> Result<(), DeploymentError> {
        let deployment = parse(&manifest())?;

        assert_eq!(deployment.kdf_domain, "Lendara Shield");
        assert_eq!(deployment.asset.code, "USDC");
        assert_eq!(deployment.asset.issuer.as_str(), USDC_ISSUER);
        assert_eq!(deployment.gvk_mode, GvkMode::Traceable);
        Ok(())
    }

    #[test]
    fn accepts_policy_flags_in_any_order() -> Result<(), DeploymentError> {
        let mut m = manifest();
        m["pools"][0]["policyFlags"] = json!(["blocklist", "allowlist"]);
        parse(&m)?;
        Ok(())
    }

    #[test]
    fn rejects_mainnet() {
        let mut m = manifest();
        m["network"] = json!("mainnet");
        assert!(matches!(parse(&m), Err(DeploymentError::NotTestnet(n)) if n == "mainnet"));
    }

    #[test]
    fn ignores_disabled_pools_but_requires_exactly_one_enabled() {
        let pool = manifest()["pools"][0].clone();
        let mut disabled = pool.clone();
        disabled["enabled"] = json!(false);

        let mut m = manifest();
        m["pools"] = json!([pool.clone(), disabled]);
        assert!(parse(&m).is_ok());

        let mut two = manifest();
        two["pools"] = json!([pool.clone(), pool]);
        assert!(matches!(parse(&two), Err(DeploymentError::PoolCount(2))));

        let mut none = manifest();
        none["pools"] = json!([]);
        assert!(matches!(parse(&none), Err(DeploymentError::PoolCount(0))));
    }

    #[test]
    fn rejects_native_asset() {
        let mut m = manifest();
        m["pools"][0]["asset"] = json!({"kind": "native"});
        assert!(matches!(parse(&m), Err(DeploymentError::NotClassicAsset)));
    }

    #[test]
    fn rejects_blocklist_only_pools() {
        let mut m = manifest();
        m["pools"][0]["policyFlags"] = json!(["blocklist"]);
        assert!(matches!(parse(&m), Err(DeploymentError::WrongPolicy)));
    }

    #[test]
    fn rejects_pools_without_a_global_view_key() {
        let mut m = manifest();
        m["pools"][0]["gvkMode"] = Value::Null;
        assert!(matches!(parse(&m), Err(DeploymentError::NoGlobalViewKey)));
    }

    #[test]
    fn rejects_invalid_addresses() {
        let mut m = manifest();
        m["asp_membership"] = json!("CNOTACONTRACT");
        assert!(matches!(parse(&m), Err(DeploymentError::Parse(_))));
    }

    #[test]
    fn reports_a_missing_file() {
        let err = PoolDeployment::load(Path::new("/nonexistent/deployments.json"));
        assert!(matches!(err, Err(DeploymentError::Read { .. })));
    }
}
