#!/usr/bin/env bash
# Serves the pool SDK to the browser: copies its JS, WASM and workers, the
# circuit artifacts of this deployment's policies and the deployment config
# into apps/web/public/spp/ (gitignored). The SDK is loaded at runtime from
# there instead of being bundled, so its workers keep their relative URLs.
#
# Usage: scripts/stage-web-sdk.sh   (reads SHIELD_DEPLOYMENT_FILE from .env)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
web="$root/apps/web"
sdk="$web/node_modules/stellar-private-payments"
out="$web/public/spp"

deployment="${SHIELD_DEPLOYMENT_FILE:-}"
if [[ -z "$deployment" && -f "$root/.env" ]]; then
  deployment="$(grep -E '^SHIELD_DEPLOYMENT_FILE=' "$root/.env" | cut -d= -f2- || true)"
fi
[[ -f "$deployment" ]] || { echo "SHIELD_DEPLOYMENT_FILE not found (run make setup-testnet)" >&2; exit 1; }
[[ -d "$sdk/dist" ]] || { echo "stellar-private-payments is not installed (run pnpm install)" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out/sdk/dist/circuits"
cp -R "$sdk/js" "$out/sdk/js"
cp "$sdk/dist/"*.js "$sdk/dist/"*.wasm "$out/sdk/dist/"
cp -R "$sdk/dist/workers" "$out/sdk/dist/workers"
cp "$sdk/dist/circuits/circuits.json" "$sdk/dist/circuits/NOTICE.txt" "$out/sdk/dist/circuits/"

# Only the circuits of the policies this pool's verifiers use (~10% of 272 MB).
for policy in $(jq -r '.verifiers | keys[]' "$deployment"); do
  cp "$sdk/dist/circuits/policy_tx_2_2_${policy}".* "$sdk/dist/circuits/policy_tx_2_2_${policy}_proving_key.bin" "$out/sdk/dist/circuits/"
done

jq 'del(.asp_membership_wasm_hash, .asp_non_membership_wasm_hash)' "$deployment" > "$out/deployments.json"
echo "staged pool SDK in ${out#"$root"/} ($(du -sh "$out" | cut -f1))"
