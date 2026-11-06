#!/usr/bin/env bash
# Checks Lendara Shield's testnet setup on-chain (`make test-testnet`).
# Read-only: it never sends a transaction. Every check prints one line and the
# script exits 1 if any of them failed.
#
#   - operator and intake accounts exist and hold XLM for fees
#   - the intake trusts the ramp asset
#   - the asset's SAC is deployed and is the pool's token
#   - the pool is allowlist-blocklist + gvk-traceable with the expected KDF domain
#   - SPP's own verify-deployment.sh passes against the recorded manifest
#   - with SHIELD_BOOTNODE_URL: the bootnode serves the pool's allow-list
#     events (or hands off to the RPC); without it: the pool is still inside
#     the RPC retention window
#
# Env: the variables written by scripts/setup-testnet.sh (see .env), and
#      SHIELD_BOOTNODE_URL (optional).

set -euo pipefail

readonly NETWORK=testnet
readonly HORIZON=https://horizon-testnet.stellar.org
readonly RPC=https://soroban-testnet.stellar.org
readonly KDF_DOMAIN="Lendara Shield"
readonly MIN_XLM=5

failures=0
pass() { printf '  ok    %s\n' "$*"; }
fail() { printf '  FAIL  %s\n' "$*"; failures=$((failures + 1)); }
check() { local label=$1; shift; if "$@" >/dev/null 2>&1; then pass "$label"; else fail "$label"; fi; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

for var in SHIELD_DEPLOYMENT_FILE SHIELD_ASSET_CONTRACT RAMP_ASSET_CODE RAMP_ASSET_ISSUER; do
  [[ -n ${!var:-} ]] || die "$var is not set; run make setup-testnet and add its output to .env"
done
readonly MANIFEST=$SHIELD_DEPLOYMENT_FILE
[[ -f $MANIFEST ]] || die "manifest not found: $MANIFEST"

has_xlm() {
  curl -fsS "$HORIZON/accounts/$(stellar keys address "$1")" |
    jq -e --argjson min "$MIN_XLM" '.balances[] | select(.asset_type == "native") | (.balance | tonumber) >= $min'
}

intake_trusts_asset() {
  curl -fsS "$HORIZON/accounts/$(stellar keys address shield-intake)" |
    jq -e --arg c "$RAMP_ASSET_CODE" --arg i "$RAMP_ASSET_ISSUER" \
      'any(.balances[]; .asset_code == $c and .asset_issuer == $i)'
}

sac_answers() {
  stellar contract invoke --id "$SHIELD_ASSET_CONTRACT" --source-account shield-operator \
    --network "$NETWORK" --send=no -- symbol
}

pool_config_matches() {
  jq -e --arg sac "$SHIELD_ASSET_CONTRACT" --arg kdf "$KDF_DOMAIN" \
    --arg c "$RAMP_ASSET_CODE" --arg i "$RAMP_ASSET_ISSUER" '
    .network == "testnet" and .kdf_domain == $kdf
      and (.pools | length) == 1
      and (.pools[0] | .tokenContractId == $sac and .enabled
        and .asset.kind == "classic" and .asset.code == $c and .asset.issuer == $i
        and .gvkMode == "traceable" and (.policyFlags | sort) == ["allowlist", "blocklist"])
  ' "$MANIFEST"
}

spp_verify() {
  local spp_dir
  spp_dir=$(dirname "$(dirname "$MANIFEST")")/spp
  "$spp_dir/deployments/scripts/verify-deployment.sh" "$NETWORK" --manifest "$MANIFEST"
}

inside_retention_window() {
  local deployed oldest
  deployed=$(jq -r '.pools[0].deploymentLedger' "$MANIFEST")
  oldest=$(curl -fsS -X POST "$RPC" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' | jq -r '.result.oldestLedger')
  ((deployed >= oldest))
}

# The bootnode answers the allow-list's events from the deployment ledger,
# or hands off (-32002) to the RPC; "not ready" (-32004) or "invalid" fail.
bootnode_serves_pool() {
  local membership deployed response
  membership=$(jq -r '.asp_membership' "$MANIFEST")
  deployed=$(jq -r '.pools[0].deploymentLedger' "$MANIFEST")
  response=$(curl -fsS -X POST "$SHIELD_BOOTNODE_URL" -H 'Content-Type: application/json' -d "$(
    jq -nc --arg c "$membership" --argjson l "$deployed" '{jsonrpc: "2.0", id: 1, method: "getEvents",
      params: {startLedger: $l, filters: [{type: "contract", topics: [["**"]], contractIds: [$c]}],
        pagination: {limit: 1}}}'
  )")
  jq -e '.result.events != null or .error.code == -32002' <<<"$response"
}

printf 'Lendara Shield testnet checks (%s)\n' "$MANIFEST"
check "operator holds at least $MIN_XLM XLM" has_xlm shield-operator
check "intake holds at least $MIN_XLM XLM" has_xlm shield-intake
check "intake trusts $RAMP_ASSET_CODE:$RAMP_ASSET_ISSUER" intake_trusts_asset
check "SAC $SHIELD_ASSET_CONTRACT answers" sac_answers
check "pool is allowlist-blocklist + gvk-traceable on the SAC" pool_config_matches
check "SPP verify-deployment.sh" spp_verify
if [[ -n ${SHIELD_BOOTNODE_URL:-} ]]; then
  check "bootnode $SHIELD_BOOTNODE_URL serves the pool's allow-list events" bootnode_serves_pool
else
  check "pool deployment ledger is inside the RPC retention window" inside_retention_window
fi

if ((failures > 0)); then
  printf '\n%d check(s) failed\n' "$failures"
  exit 1
fi
printf '\nall checks passed\n'
