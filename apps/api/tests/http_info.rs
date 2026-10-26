//! `GET /v1/info` against the contract's `Info` schema.

mod common;

use axum::http::StatusCode;
use common::{TestResult, app, app_with_ramp, get, send, unreachable_pool};
use serde_json::json;

#[tokio::test]
async fn describes_the_pool_and_the_enabled_modes() -> TestResult {
    let (status, _, body) = send(app(unreachable_pool()?)?, get("/v1/info")?).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "network": "testnet",
            "network_passphrase": "Test SDF Network ; September 2015",
            "pool": {
                "contract_id": "CCAEA4MKORUJV3G3V47SKWLJKQZBYGGFU42JSCXNUIJIZMNN2MSDLFMO",
                "asset": {
                    "code": "USDC",
                    "issuer": "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5",
                    "contract_id": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
                },
                "asp_policy": ["allowlist", "blocklist"],
                "asp_membership_contract": "CBXCTA27XVLULCT4O5LCE2ETKW7A6PXEOFXOKHHFP4JCPRT72T7HBQLL",
                "asp_non_membership_contract": "CBGH7D7JZ3FRJ6ILEDRABBUWTEEV42CKBWTPQQWMHNF7H3OXWREEOAFA",
                "gvk_mode": "traceable",
                "kdf_domain": "Lendara Shield"
            },
            "modes": {
                "ramp": {"enabled": true, "anchor_home_domain": "lendara-monorepo-fixh.vercel.app"},
                "direct": {"enabled": true}
            }
        })
    );
    Ok(())
}

#[tokio::test]
async fn reports_ramp_mode_disabled_without_an_anchor() -> TestResult {
    let (status, _, body) =
        send(app_with_ramp(unreachable_pool()?, None)?, get("/v1/info")?).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["modes"]["ramp"],
        json!({"enabled": false, "anchor_home_domain": null})
    );
    assert_eq!(body["modes"]["direct"]["enabled"], true);
    Ok(())
}

#[tokio::test]
async fn does_not_need_the_database() -> TestResult {
    // Info is computed at startup: it must keep working during a DB outage.
    let (status, _, _) = send(app(unreachable_pool()?)?, get("/v1/info")?).await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}
