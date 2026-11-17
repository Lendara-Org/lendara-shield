#!/usr/bin/env bash
# Deploys a commit of `main` to the demo server: builds it in its own release
# directory, points /home/shield/current at it and restarts what changed,
# rolling back if anything in the cutover fails. See docs/operations/deploy.md.
#
# Installed root-owned as /usr/local/sbin/shield-deploy (a change here is not
# live until it is installed again). Run as root:
#   shield-deploy <commit>              the commit must be origin/main's head
#   shield-deploy --manual <commit>     by a person, after the steps in
#                                       docs/operations/deploy.md: allows
#                                       pool-cli and migration changes, the
#                                       same commit again, and always restarts
#                                       the API
#   shield-deploy --bootstrap <commit>  first run: builds and creates current,
#                                       restarts nothing
#
# Root only takes the lock, reads the custody count and controls the services.
# Everything that touches files the build can write (the release, the symlink,
# pruning) runs as `shield`, the user the build already runs as.
set -euo pipefail
umask 022

readonly SHIELD_HOME=/home/shield
readonly REPO=$SHIELD_HOME/lendara-shield
readonly ENV_FILE=$REPO/.env
readonly RELEASES=$SHIELD_HOME/deploys
readonly CURRENT=$SHIELD_HOME/current
readonly CARGO_TARGET=$SHIELD_HOME/cargo-target/api
readonly KEEP_RELEASES=3
readonly CUSTODY_WAIT_SECONDS=900
readonly API_URL=http://127.0.0.1:8080
readonly WEB_URL=http://127.0.0.1:3000

log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
die() { log "ERROR: $*"; exit 1; }

as_shield() {
  runuser -u shield -- env -i HOME=$SHIELD_HOME \
    PATH=$SHIELD_HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin CI=true "$@"
}

# Intents whose money is in Shield's custody (AGENTS.md §6.4).
custody_count() {
  echo "select count(*) from intents where status in ('funds_received','shielding','transferring');" |
    docker exec -i lendara-shield-postgres-1 sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -At'
}

# Stops the API only while no intent is in custody and no pool-cli runs. A
# payment can still arrive in the second between the check and the stop; the
# API's shutdown is safe then (every step stored its state before any network
# call, AGENTS.md §5.3), so the window only delays that intent.
stop_api_when_idle() {
  local waited=0
  while :; do
    if [[ "$(custody_count)" == 0 ]] && ! pgrep -x pool-cli >/dev/null; then
      if ! systemctl stop shield-api; then
        log "could not stop shield-api"
        return 1
      fi
      log "API stopped with no intent in custody"
      return 0
    fi
    if ((waited >= CUSTODY_WAIT_SECONDS)); then
      log "intents still in custody after ${CUSTODY_WAIT_SECONDS}s"
      return 1
    fi
    log "intents in custody or pool-cli running; waiting"
    sleep 20
    ((waited += 20))
  done
}

healthy() {
  local api=$1
  for _ in $(seq 1 30); do
    if curl -fs -o /dev/null --max-time 5 "$WEB_URL/en" &&
      { [[ $api == no ]] ||
        curl -fs --max-time 5 "$API_URL/health" | grep -q '"status":"ok"'; }; then
      return 0
    fi
    sleep 2
  done
  return 1
}

switch_to() {
  as_shield ln -sfn "$1" "$CURRENT.new" && as_shield mv -T "$CURRENT.new" "$CURRENT"
}

# Puts the previous release back. Every step runs even if one fails.
rollback() {
  local ok=yes api_stopped=no
  log "rolling back to $prev ($1)"
  if [[ $restart_api == yes ]]; then
    if stop_api_when_idle; then
      api_stopped=yes
    else
      log "the API still runs the new release; restart shield-api when no intent is in custody"
      ok=no
    fi
  fi
  if switch_to "$prev_dir"; then
    # The old API starts again only from the old release.
    if [[ $api_stopped == yes ]]; then
      systemctl start shield-api || ok=no
    fi
  else
    ok=no
    [[ $api_stopped == yes ]] && log "shield-api left stopped: current could not be switched back"
  fi
  systemctl restart shield-web || ok=no
  if [[ $ok == yes ]] && healthy "$restart_api"; then
    die "rolled back to $prev"
  fi
  die "rollback to $prev incomplete; check journalctl -u shield-web -u shield-api"
}

mode=auto
case ${1:-} in
  --bootstrap) mode=bootstrap; shift ;;
  --manual) mode=manual; shift ;;
esac
sha=${1:-}
[[ $sha =~ ^[0-9a-f]{40}$ ]] || die "usage: shield-deploy [--manual|--bootstrap] <40-hex commit>"
[[ $EUID == 0 ]] || die "run as root"
# Over SSH the script starts in /home/deploy, which `shield` cannot enter.
cd /

# Keep going if the SSH session that started the deploy drops: a half-done
# cutover is worse than a finished one. Everything also goes to the log.
trap '' HUP PIPE
exec > >(tee -p -a /var/log/shield-deploy.log) 2>&1

exec 9>/run/lock/shield-deploy.lock
flock -n 9 || die "another deploy is running"

as_shield git -C "$REPO" fetch --quiet origin main
head=$(as_shield git -C "$REPO" rev-parse origin/main)
if [[ $sha != "$head" ]]; then
  log "skipped: $sha is not main's head ($head); that commit's own run deploys it"
  exit 0
fi

# The services read paths from .env with the release as working directory: a
# relative one would point inside a release that does not have that file.
relative=$(sed -nE "s/^[[:space:]]*(export[[:space:]]+)?([A-Z0-9_]*(PATH|FILE|DIR))=[\"']?([^\"'/[:space:]#][^\"']*)?.*/\2 \4/p" "$ENV_FILE" |
  awk 'NF >= 2 { print $1 }' || true)
[[ -z $relative ]] || die "make these .env paths absolute first: $(tr '\n' ' ' <<<"$relative")"

prev_dir=""
if [[ $mode == bootstrap ]]; then
  [[ ! -e $CURRENT ]] || die "$CURRENT exists; bootstrap is only for the first run"
  # Compare with what the server checkout (the running services) is at.
  prev=$(as_shield git -C "$REPO" rev-parse HEAD)
else
  [[ -L $CURRENT ]] || die "$CURRENT does not exist; run the bootstrap from docs/operations/deploy.md"
  prev_dir=$(readlink -f "$CURRENT")
  prev=$(as_shield cat "$prev_dir/.commit")
  [[ $prev =~ ^[0-9a-f]{40}$ ]] || die "the current release has no valid .commit"
  if [[ $sha == "$prev" && $mode == auto ]]; then
    log "already deployed: $sha"
    exit 0
  fi
fi

changed=$(as_shield git -C "$REPO" diff --name-only "$prev" "$sha")
delicate=$(grep -E '^(apps/pool-cli/|apps/api/migrations/)' <<<"$changed" || true)
if [[ -n $delicate && $mode != manual ]]; then
  die "needs the manual procedure (docs/operations/deploy.md), changed: $(tr '\n' ' ' <<<"$delicate")"
fi
build_api=no
if [[ $mode == bootstrap ]] || grep -q '^apps/api/' <<<"$changed"; then
  build_api=yes
fi
restart_api=$build_api
[[ $mode == manual ]] && restart_api=yes
if grep -q '^deploy/' <<<"$changed"; then
  log "note: deploy/ changed; install it by hand if needed (docs/operations/deploy.md)"
fi

rel=$RELEASES/${sha:0:12}-$(date -u +%Y%m%d%H%M%S)
log "building $sha in $rel (build API: $build_api, restart API: $restart_api)"
as_shield mkdir -p "$RELEASES" "$rel/bin"
# A build that fails leaves nothing behind; cleared once the release is complete.
trap 'as_shield rm -rf -- "$rel"' EXIT
as_shield bash -c "git -C '$REPO' archive '$sha' | tar -x -C '$rel'"
as_shield ln -s "$ENV_FILE" "$rel/.env"

as_shield bash -c "cd '$rel' && pnpm install --frozen-lockfile --prefer-offline --ignore-scripts >/dev/null && make stage-web-sdk && make build-js >/dev/null"
log "web built"

if [[ $build_api == yes ]]; then
  as_shield bash -c "CARGO_TARGET_DIR='$CARGO_TARGET' cargo build --quiet --manifest-path '$rel/apps/api/Cargo.toml' --release --locked &&
    install -m 755 '$CARGO_TARGET/release/shield-api' '$rel/bin/shield-api'"
  log "API built"
else
  as_shield install -m 755 "$prev_dir/bin/shield-api" "$rel/bin/shield-api"
fi
as_shield bash -c "echo '$sha' > '$rel/.commit'"
trap - EXIT

if [[ $mode == bootstrap ]]; then
  switch_to "$rel"
  log "bootstrap: $CURRENT -> $rel; install the systemd drop-ins and restart (docs/operations/deploy.md)"
  exit 0
fi

if [[ $restart_api == yes ]]; then
  stop_api_when_idle || die "nothing changed; run the deploy again later"
fi
# `!` keeps errexit out of the cutover, so any failing step reaches rollback.
if ! { switch_to "$rel" &&
  systemctl restart shield-web &&
  { [[ $restart_api == no ]] || systemctl start shield-api; } &&
  healthy "$restart_api"; }; then
  rollback "the cutover or the health checks failed"
fi
if [[ $restart_api == yes ]] && ! curl -fsS --max-time 5 "$API_URL/v1/info" | grep -q '"max_deposit":"[0-9]'; then
  log "warning: /v1/info has no max_deposit (RPC unreachable at startup?); restart shield-api once the RPC answers"
fi
log "deployed $sha"

# Keep the current release and the newest others made by this script.
as_shield find "$RELEASES" -mindepth 2 -maxdepth 2 -name .commit -printf '%T@ %h\n' | sort -rn | cut -d' ' -f2 |
  grep -vxF "$rel" | tail -n +$((KEEP_RELEASES + 1)) | while read -r old; do
  log "removing old release $old"
  as_shield rm -rf -- "$old"
done
