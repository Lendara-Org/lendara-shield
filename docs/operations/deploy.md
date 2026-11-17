# Deploying the demo server

**Merging to `main` deploys.** When every CI check passes on a push to `main`,
the `Deploy to the demo server` job (`.github/workflows/ci.yml`) connects to the
server over SSH and asks it to deploy that commit. Nobody needs server access
for an ordinary change. Pushes to `develop` and pull requests never deploy.

## What the server does

`deploy/deploy.sh` (installed as `/usr/local/sbin/shield-deploy`):

1. Refuses anything but `main`'s head (it fetches `origin` itself) and skips a
   commit already deployed. One deploy at a time (`flock`).
2. Stops without touching the running services if the commit changes
   `apps/pool-cli/` or `apps/api/migrations/` (those need the manual procedure
   below), or if a `*_PATH`, `*_FILE` or `*_DIR` in `.env` is relative (the
   services run inside the release directory).
3. Builds the commit in its own directory, `/home/shield/deploys/<commit>-<time>`,
   as the `shield` user, while the running site stays up: the web
   (`pnpm install --ignore-scripts`: the only install script is the git hook
   setup; `make stage-web-sdk`, `make build-js`) always, the API only
   if `apps/api/` changed (otherwise the current binary is reused).
4. If the API changed, waits (up to 15 minutes) until no intent is in custody
   (`funds_received`, `shielding`, `transferring`) and no `pool-cli` runs, then
   stops it. If intents stay in custody, it gives up with nothing changed; run
   the job again later. A payment can still arrive in the second between the
   check and the stop: the API's shutdown is built for that (every step stores
   its state before any network call, AGENTS.md §5.3), so that intent only
   resumes after the restart.
5. Points `/home/shield/current` at the new release and restarts the web (and
   starts the API). The systemd units run whatever `current` points to
   (`deploy/systemd/`).
6. Checks the web and `/health`. If any cutover step or check fails, it points
   `current` back to the previous release and restarts it, stopping the new API
   the same careful way. A missing `max_deposit` after an API restart is reported
   as a warning (RPC unreachable at startup), not a failure.
7. Keeps the current release and the three newest others it made.

Root only takes the lock, counts intents in custody and controls the services;
everything that touches the release files runs as `shield`. Output goes to the
job log and to `/var/log/shield-deploy.log`. The deploy keeps going if the job is
cancelled or the SSH connection drops.

## Access and keys

- **GitHub:** the `demo` environment (Settings → Environments) only allows
  `main`. It holds the secret `DEPLOY_SSH_KEY` (a private key used for nothing
  else) and the variables `DEPLOY_HOST` and `DEPLOY_KNOWN_HOSTS` (the server's
  public host keys, so the job refuses an impostor). GitHub never shows the
  secret again, but a workflow running on `main` can use it: **whoever can merge
  to `main` is trusted to deploy**, and review should treat workflow changes as
  sensitive.
- **Server:** the key belongs to the `deploy` user, which has no password and no
  shell access: its `authorized_keys` entry is
  `restrict,command="/usr/local/sbin/shield-deploy-ssh"`, so the only thing it can
  send is a commit hash. `deploy/deploy-ssh.sh` checks it is 40 hex characters
  and runs `sudo -n /usr/local/sbin/shield-deploy <commit>`, the only command the
  `deploy` user may run as root (`deploy/sudoers-deploy`). The deployed code
  runs as `shield`, as before.
- The scripts in `/usr/local/sbin` are root-owned copies: changing them in the
  repo does not change the server until someone installs them again
  (`install -o root -g root -m 755 deploy/deploy.sh /usr/local/sbin/shield-deploy`,
  same for `deploy-ssh.sh` → `shield-deploy-ssh`). The deploy notes when
  `deploy/` changed.
- **Revoking:** delete the `deploy` user's `authorized_keys` line (or the
  secret). To rotate, generate a new key, replace both, and never print it.

## When the deploy refuses or fails

- **`needs the manual procedure`:** the commit changes the pool CLI or the
  database. A person with server access deploys it (below).
- **`intents still in custody`:** re-run the failed job from the Actions tab
  later.
- **`rolled back`:** the site runs the previous release; the job log and
  `journalctl -u shield-web -u shield-api` say why. Fix it in a new PR.
- **`rollback … incomplete`:** someone with server access must look now.
- **`skipped: … is not main's head`:** a newer commit reached `main`; its own run
  deploys it.

## Manual procedure

Needs SSH access as an administrator. `shield-deploy --manual <commit>` (with
main's head) runs the same steps without the pool CLI and migration guard,
accepts the commit already deployed (to rebuild it) and always restarts the API.

- **Migrations:** the API applies them when it starts, and the previous binary
  refuses a database with migrations it does not know, so the automatic
  rollback cannot undo them. Stop `shield-api` with no intent in custody, back up
  the database (`pg_dump`, mode 600, outside the repo, with a checksum), then run
  `shield-deploy --manual <commit>` (it starts the API) and check the log for
  `database migrations applied`. Restoring that backup later would lose whatever
  the API recorded after it (intents, payments, stored transaction hashes), and
  a lost hash can lead to a second pool deposit. Before any restore: stop the
  API with no intent in custody, dump the current database too, and carry over
  or reconcile everything recorded after the backup against the network before
  starting the API again.
- **Pool CLI:** the deploy never builds it; the API runs `POOL_CLI_PATH` from
  `/home/shield/lendara-shield/.env`. Follow AGENTS.md §9 (wallet schema
  upgrades) and `docs/testing/2026-11-12-native-sqlite.md`: stop wallet
  consumers, back up the intake wallet with its sidecars, build `pool-cli` in its
  own directory, point `POOL_CLI_PATH` at it (absolute), then run
  `shield-deploy --manual <commit>`.
- **`.env` changes:** edit it, then `shield-deploy --manual <commit>` rebuilds the
  web (the `NEXT_PUBLIC_*` variables are baked in at build time) and restarts the
  API.

Caddy (`deploy/Caddyfile`, see AGENTS.md §9) and the systemd drop-ins in
`deploy/systemd/` are installed by hand.

## One-time setup

Done once, as root, from a checkout of `main`:

```bash
install -o root -g root -m 755 deploy/deploy.sh /usr/local/sbin/shield-deploy
install -o root -g root -m 755 deploy/deploy-ssh.sh /usr/local/sbin/shield-deploy-ssh
install -o root -g root -m 440 deploy/sudoers-deploy /etc/sudoers.d/shield-deploy
visudo -cf /etc/sudoers.d/shield-deploy
useradd --system --create-home --shell /bin/bash deploy   # no password: key only
# /home/deploy/.ssh/authorized_keys (700/600, owned by deploy):
#   restrict,command="/usr/local/sbin/shield-deploy-ssh" ssh-ed25519 AAAA… github-deploy
shield-deploy --bootstrap "$(git rev-parse origin/main)"   # builds, creates current
for u in shield-api shield-web; do
  install -d /etc/systemd/system/$u.service.d
  install -m 644 deploy/systemd/$u.conf /etc/systemd/system/$u.service.d/20-current.conf
done
systemctl daemon-reload   # then restart both, with no intent in custody
```

The bootstrap compares the commit with the server checkout's `HEAD` (what the
services run) and refuses pool CLI or migration changes, always builds the API,
and restarts nothing: the services keep their old paths until the drop-ins are
installed and they restart.
