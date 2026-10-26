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
  submit) echo "{\"hash\":\"$hash\"}" ;;
  status) echo '{"status":"failed","result_xdr":"AAAA"}' ;;
  *) echo '{"error":"invalid_arguments","message":"unknown","retryable":false}'; exit 1 ;;
esac
