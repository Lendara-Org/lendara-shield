#!/usr/bin/env bash
# Sets up Lendara Shield on Stellar TESTNET. Idempotent: every step checks the
# current state first and only does what is missing, so it is safe to re-run.
#
#   1. Operator (pool + ASP admin) and intake (omnibus) identities, funded.
#   2. Intake trustline to the ramp asset (the Lendara sandbox USDC).
#   3. Stellar Asset Contract (SAC) for that asset.
#   4. Global View Key (Baby JubJub keypair) for the auditor.
#   5. Stellar Private Payments pool: allowlist-blocklist + gvk-traceable,
#      deployed with SPP's own deploy.sh at a pinned release.
#   6. Writes the resulting variables to $STATE_DIR/testnet.env.
#
# Secrets never touch the repository: identities live in the Stellar CLI
# config and everything else under $STATE_DIR (mode 700).
#
# Usage: scripts/setup-testnet.sh   (or `make setup-testnet`)
# Env:   RAMP_ASSET_ISSUER (required), RAMP_ASSET_CODE (default USDC),
#        SHIELD_STATE_DIR (default ~/.local/share/lendara-shield/testnet),
#        FORCE_REDEPLOY=1 to deploy a new pool even if one is recorded.

set -euo pipefail

readonly NETWORK=testnet
readonly HORIZON=https://horizon-testnet.stellar.org
readonly EXPLORER=https://stellar.expert/explorer/testnet

# SPP release pinned in docs/spikes/spp.md: contracts, Rust crate and npm match.
readonly SPP_REPO=https://github.com/NethermindEth/stellar-private-payments.git
readonly SPP_TAG=sdk-v0.2.1
readonly SPP_COMMIT=30d8af7de55302c48feef428168a0c5f25e84f18

readonly OPERATOR=shield-operator
readonly INTAKE=shield-intake
readonly KDF_DOMAIN="Lendara Shield"
readonly ASP_LEVELS=10
readonly POOL_LEVELS=20
# 1,000 units of a 7-decimal asset, in base units.
readonly MAX_DEPOSIT=10000000000

readonly ASSET_CODE=${RAMP_ASSET_CODE:-USDC}
readonly ASSET_ISSUER=${RAMP_ASSET_ISSUER:-}
readonly STATE_DIR=${SHIELD_STATE_DIR:-$HOME/.local/share/lendara-shield/testnet}
readonly SPP_DIR=$STATE_DIR/spp
readonly GVK_DIR=$STATE_DIR/gvk
readonly DEPLOYMENT_DIR=$STATE_DIR/deployment
readonly ENV_FILE=$STATE_DIR/testnet.env

step() { printf '\n==> %s\n' "$*"; }
ok() { printf '    ok: %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

require_tools() {
  local tool
  for tool in stellar jq curl git cargo; do
    command -v "$tool" >/dev/null || die "$tool is required"
  done
  local major
  major=$(stellar --version | head -1 | sed -E 's/^stellar ([0-9]+).*/\1/')
  ((major >= 27)) || die "stellar CLI >= 27 is required (found $major)"
  [[ $ASSET_ISSUER =~ ^G[A-Z2-7]{55}$ ]] || die "RAMP_ASSET_ISSUER must be the G... issuer of the ramp asset"
}

horizon_account() { curl -fsS "$HORIZON/accounts/$1" 2>/dev/null; }

ensure_identity() {
  local name=$1 address
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    stellar keys generate "$name" --network "$NETWORK" >/dev/null
  fi
  address=$(stellar keys address "$name")
  if ! horizon_account "$address" >/dev/null; then
    stellar keys fund "$name" --network "$NETWORK" >/dev/null
  fi
  ok "$name $address"
}

ensure_trustline() {
  local intake account
  intake=$(stellar keys address "$INTAKE")
  account=$(horizon_account "$intake") || die "cannot read $intake from Horizon"
  if ! jq -e --arg c "$ASSET_CODE" --arg i "$ASSET_ISSUER" \
    'any(.balances[]; .asset_code == $c and .asset_issuer == $i)' <<<"$account" >/dev/null; then
    stellar tx new change-trust --source-account "$INTAKE" --line "$ASSET_CODE:$ASSET_ISSUER" \
      --network "$NETWORK" >/dev/null
  fi
  ok "$INTAKE trusts $ASSET_CODE:$ASSET_ISSUER"
}

sac_is_deployed() {
  stellar contract invoke --id "$1" --source-account "$OPERATOR" --network "$NETWORK" \
    --send=no -- decimals >/dev/null 2>&1
}

ensure_sac() {
  SAC_ID=$(stellar contract id asset --asset "$ASSET_CODE:$ASSET_ISSUER" --network "$NETWORK")
  if ! sac_is_deployed "$SAC_ID"; then
    stellar contract asset deploy --asset "$ASSET_CODE:$ASSET_ISSUER" \
      --source-account "$OPERATOR" --network "$NETWORK" >/dev/null
    sac_is_deployed "$SAC_ID" || die "SAC $SAC_ID did not deploy"
  fi
  ok "SAC $SAC_ID"
}

ensure_spp_checkout() {
  if [[ ! -d $SPP_DIR/.git ]]; then
    git clone --quiet --depth 1 --branch "$SPP_TAG" "$SPP_REPO" "$SPP_DIR" 2>/dev/null
  fi
  [[ $(git -C "$SPP_DIR" rev-parse HEAD) == "$SPP_COMMIT" ]] ||
    die "$SPP_DIR is not at $SPP_TAG ($SPP_COMMIT); delete it and re-run"
  ok "SPP $SPP_TAG at $SPP_DIR"
}

ensure_gvk() {
  if [[ ! -f $GVK_DIR/admin-d.json ]]; then
    mkdir -p "$GVK_DIR" && chmod 700 "$GVK_DIR"
    (cd "$SPP_DIR" && cargo run --quiet --release -p gvkey-gen -- generate \
      --out-file "$GVK_DIR/admin-d.json" >"$GVK_DIR/admin-pub.json")
    chmod 600 "$GVK_DIR"/*.json
  fi
  ok "Global View Key $GVK_DIR/admin-d.json (keep it secret, it cannot be recovered)"
}

recorded_pool_matches() {
  local manifest=$DEPLOYMENT_DIR/deployments.json
  [[ -f $manifest ]] || return 1
  jq -e --arg sac "$SAC_ID" --arg c "$ASSET_CODE" --arg i "$ASSET_ISSUER" '
    .pools[0] | .tokenContractId == $sac and .asset.code == $c and .asset.issuer == $i
      and .gvkMode == "traceable" and (.policyFlags | sort) == ["allowlist", "blocklist"]
  ' "$manifest" >/dev/null
}

ensure_pool() {
  if [[ ${FORCE_REDEPLOY:-0} != 1 ]] && recorded_pool_matches; then
    ok "pool already recorded in $DEPLOYMENT_DIR (FORCE_REDEPLOY=1 to replace it)"
    return
  fi
  # deploy.sh compiles the contracts first: the first run takes several minutes.
  (cd "$SPP_DIR" && deployments/scripts/deploy.sh "$NETWORK" \
    --deployer "$OPERATOR" \
    --asp-levels "$ASP_LEVELS" --pool-levels "$POOL_LEVELS" \
    --max-deposit "$MAX_DEPOSIT" --kdf-domain "$KDF_DOMAIN" \
    --pool "allowlist-blocklist:gvk-traceable:classic:$ASSET_CODE:$ASSET_ISSUER:$SAC_ID" \
    --gvk-authority-pubkey-file "$GVK_DIR/admin-pub.json")
  mkdir -p "$DEPLOYMENT_DIR"
  cp "$SPP_DIR/deployments/$NETWORK/deployments.json" "$SPP_DIR/deployments/$NETWORK/circuits.json" \
    "$DEPLOYMENT_DIR/"
  recorded_pool_matches || die "deploy.sh finished but the manifest does not match the expected pool"
  ok "pool $(jq -r '.pools[0].poolContractId' "$DEPLOYMENT_DIR/deployments.json")"
}

write_env() {
  local manifest=$DEPLOYMENT_DIR/deployments.json
  umask 077
  cat >"$ENV_FILE" <<EOF
# Generated by scripts/setup-testnet.sh on $(date -u +%Y-%m-%dT%H:%M:%SZ). TESTNET ONLY.
# Contains secrets: copy into the repo's .env (git-ignored), never commit.
SHIELD_OPERATOR_SECRET=$(stellar keys secret "$OPERATOR")
SHIELD_INTAKE_SECRET=$(stellar keys secret "$INTAKE")
SHIELD_INTAKE_WALLET_PATH=$STATE_DIR/intake-wallet.sqlite
SHIELD_GVK_KEY_FILE=$GVK_DIR/admin-d.json
SHIELD_DEPLOYMENT_FILE=$manifest
SHIELD_CIRCUITS_LOCK_FILE=$DEPLOYMENT_DIR/circuits.json
SHIELD_CIRCUITS_DIR=$STATE_DIR/circuits
SHIELD_POOL_CONTRACT=$(jq -r '.pools[0].poolContractId' "$manifest")
SHIELD_ASSET_CONTRACT=$SAC_ID
RAMP_ASSET_CODE=$ASSET_CODE
RAMP_ASSET_ISSUER=$ASSET_ISSUER
EOF
}

summary() {
  local manifest=$DEPLOYMENT_DIR/deployments.json
  step "Done"
  jq -r --arg e "$EXPLORER" '
    "    pool:               \(.pools[0].poolContractId)",
    "    asset (SAC):        \(.pools[0].tokenContractId)",
    "    ASP membership:     \(.asp_membership)",
    "    ASP non-membership: \(.asp_non_membership)",
    "    admin (operator):   \(.admin)",
    "    explorer:           \($e)/contract/\(.pools[0].poolContractId)"
  ' "$manifest"
  cat <<EOF

    Variables written to $ENV_FILE (mode 600, contains secrets).
    Add them to the repo's .env with:

      grep -v '^#' $ENV_FILE >> .env

    Then check everything on-chain with: make test-testnet
EOF
}

main() {
  require_tools
  mkdir -p "$STATE_DIR" && chmod 700 "$STATE_DIR"

  step "Identities ($NETWORK)"
  ensure_identity "$OPERATOR"
  ensure_identity "$INTAKE"
  step "Trustline"
  ensure_trustline
  step "Stellar Asset Contract"
  ensure_sac
  step "Stellar Private Payments $SPP_TAG"
  ensure_spp_checkout
  ensure_gvk
  ensure_pool
  write_env
  summary
}

main "$@"
