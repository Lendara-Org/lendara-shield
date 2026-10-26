#!/usr/bin/env bash
# Stand-in for apps/pool-cli in the adapter tests: answers each command with
# fixed JSON. FAKE_MODE (forwarded through SHIELD_CIRCUITS_DIR) picks a
# failure to simulate.
set -euo pipefail

mode=${SHIELD_CIRCUITS_DIR:-ok}
hash=$(printf 'ab%.0s' $(seq 32))

case "$mode" in
  garbage) echo "not json"; exit 0 ;;
  crash) exit 3 ;;
  slow) sleep 5 ;;
  error) echo '{"error":"not_allowed","message":"ASP membership sync required","retryable":false}'; exit 1 ;;
esac

case "$1" in
  prepare-deposit | prepare-transfer)
    # Proves the arguments arrived and no unexpected variable leaked in.
    leaked=${ADMIN_TOKEN:-none}
    echo "{\"hash\":\"$hash\",\"envelope_xdr\":\"$1:$3:$leaked\",\"valid_until\":null,\"sequence\":42}"
    ;;
  keys)
    # "unlisted" answers a leaf that is not in the recorded allow-list events.
    leaf=0x2e20c54a6631dec2c7a35dfcca7f15907a81bac53c6b7a959bb21521a10b20b7
    [[ "$mode" == unlisted ]] && leaf=0x$(printf '11%.0s' $(seq 32))
    echo "{\"account\":\"GA\",\"note_public_key\":\"0x$(printf '22%.0s' $(seq 32))\",\"encryption_public_key\":\"0x$(printf '33%.0s' $(seq 32))\",\"asp_leaf\":\"$leaf\"}"
    ;;
  submit) echo "{\"hash\":\"$hash\"}" ;;
  status) echo '{"status":"failed","result_xdr":"AAAA"}' ;;
  audit)
    # Real output shape; the owner is a note key as the SDK's audit prints it.
    [[ "$3" == "2" && "${SHIELD_GVK_KEY_FILE:-}" == /gvk.json ]] || { echo '{"error":"config","message":"no gvk","retryable":false}'; exit 1; }
    owner=0x24382f33fe99fcea57632e6dd4ba6eca4d7053785ff10b71e90577541cd7a6a2
    echo "{\"transactions\":[{\"ledger\":5392292,\"inputs\":[{\"owner\":\"$owner\",\"amount\":\"20000000\"}],\"outputs\":[{\"owner\":\"$owner\",\"amount\":\"5000000\"}]},{\"ledger\":5392288,\"inputs\":[],\"outputs\":[]}]}"
    ;;
  *) echo '{"error":"invalid_arguments","message":"unknown","retryable":false}'; exit 1 ;;
esac
