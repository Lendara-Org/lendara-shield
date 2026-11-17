#!/usr/bin/env bash
# Forced command of the `deploy` user's SSH key (see docs/operations/deploy.md):
# the key can only ask for a deploy of one commit, nothing else. Installed
# root-owned as /usr/local/sbin/shield-deploy-ssh.
set -euo pipefail
sha=${SSH_ORIGINAL_COMMAND:-}
if [[ ! $sha =~ ^[0-9a-f]{40}$ ]]; then
  echo "expected a 40-hex commit" >&2
  exit 2
fi
exec sudo -n /usr/local/sbin/shield-deploy "$sha"
