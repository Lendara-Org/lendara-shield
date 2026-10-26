#!/usr/bin/env bash
# Initialises Shield's intake for the pool. TESTNET ONLY. Idempotent: safe to
# run on every deploy and on a live server.
#
# A pool deposit from the intake needs its privacy keys in the SDK wallet and
# its ASP leaf in the allow-list; deploying the pool does neither, and without
# them every funded ramp intent ends in `error`. This runs
# `shield-api init-intake`, which derives the keys if missing and, only if the
# leaf is absent, inserts it signed by the operator and waits for the
# transaction.
#
# Usage: scripts/init-intake.sh [ENV_FILE]   (default: .env at the repo root)
# Needs the release builds: make build-api build-pool-cli
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
readonly ROOT
readonly ENV_FILE=${1:-$ROOT/.env}
readonly SHIELD_API=${SHIELD_API_BIN:-$ROOT/apps/api/target/release/shield-api}

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[[ -f $ENV_FILE ]] || die "$ENV_FILE not found"
[[ -x $SHIELD_API ]] || die "$SHIELD_API not found: run make build-api"

(
  # Don't `source` the file (AGENTS.md §9): values have spaces and `;`.
  # `export "NAME=value"` takes each line literally, and secrets never reach
  # a command line.
  while IFS= read -r line || [[ -n $line ]]; do
    [[ $line =~ ^[A-Za-z_][A-Za-z0-9_]*= ]] && export "$line"
  done <"$ENV_FILE"
  # init-intake never connects to the database, but the config requires it.
  export DATABASE_URL=${DATABASE_URL:-postgres://unused}
  export POOL_CLI_PATH=${POOL_CLI_PATH:-$ROOT/apps/pool-cli/target/release/pool-cli}
  export LOG_FORMAT=${LOG_FORMAT:-pretty}
  # Relative paths in .env are relative to apps/api, as for the service.
  cd "$ROOT/apps/api"
  exec "$SHIELD_API" init-intake
)
