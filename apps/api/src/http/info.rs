//! `GET /v1/info`: public configuration the web needs (contract `Info`).
//!
//! Built once at startup from the validated deployment; the handler only
//! serves the cached value.

use std::sync::Arc;

use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    deployment::{GvkMode, PoolDeployment, TESTNET},
    domain::{AccountId, ContractId},
    state::AppState,
};

const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InfoResponse {
    network: &'static str,
    network_passphrase: &'static str,
    pool: PoolInfo,
    modes: Modes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PoolInfo {
    contract_id: ContractId,
    asset: AssetInfo,
    asp_policy: [&'static str; 2],
    asp_membership_contract: ContractId,
    asp_non_membership_contract: ContractId,
    gvk_mode: &'static str,
    kdf_domain: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AssetInfo {
    code: String,
    issuer: AccountId,
    contract_id: ContractId,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct Modes {
    ramp: RampMode,
    direct: DirectMode,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct RampMode {
    enabled: bool,
    anchor_home_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct DirectMode {
    enabled: bool,
}

impl InfoResponse {
    #[must_use]
    pub fn new(deployment: &PoolDeployment, ramp_anchor_home_domain: Option<String>) -> Self {
        Self {
            network: TESTNET,
            network_passphrase: TESTNET_PASSPHRASE,
            pool: PoolInfo {
                contract_id: deployment.pool.clone(),
                asset: AssetInfo {
                    code: deployment.asset.code.clone(),
                    issuer: deployment.asset.issuer.clone(),
                    contract_id: deployment.asset.contract.clone(),
                },
                // `PoolDeployment` only accepts allowlist-blocklist pools.
                asp_policy: ["allowlist", "blocklist"],
                asp_membership_contract: deployment.asp_membership.clone(),
                asp_non_membership_contract: deployment.asp_non_membership.clone(),
                gvk_mode: match deployment.gvk_mode {
                    GvkMode::ViewOnly => "view_only",
                    GvkMode::Traceable => "traceable",
                },
                kdf_domain: deployment.kdf_domain.clone(),
            },
            modes: Modes {
                ramp: RampMode {
                    enabled: ramp_anchor_home_domain.is_some(),
                    anchor_home_domain: ramp_anchor_home_domain,
                },
                // Direct mode needs nothing beyond the pool: always on.
                direct: DirectMode { enabled: true },
            },
        }
    }
}

pub async fn info(State(state): State<AppState>) -> Json<Arc<InfoResponse>> {
    Json(state.info)
}
