# Moving the demo to a new domain

The demo runs at `lendara-shield.duckdns.org` (plus the old alias `64-226-124-221.sslip.io`). This is the checklist for moving it to a new domain (for example a `.xyz`) without breaking old links. Replace `NEW` with the new name.

## Before

1. **DNS:** point an `A` record for `NEW` to the server (`64.226.124.221`). Check it resolves (`dig +short NEW`) before touching Caddy: Caddy gets the HTTPS certificate on its own (Let's Encrypt), but only once the name points to the server.
2. **Keep the DuckDNS name pointing to the server.** It will redirect to `NEW`. It has HSTS for a year (`deploy/Caddyfile`), so browsers that visited it only reach it over HTTPS: Caddy keeps its certificate renewed as long as it stays configured.

## Order matters

Serve **both** names first, switch the configuration, verify, and only then turn the old names into redirects. A redirect answers the browser SDK's JSON-RPC `POST` to the bootnode with a 301, which browsers retry as a `GET` (405): pages still built for the old name, or tabs left open, would stop shielding.

1. Caddy: add `NEW` to the existing site block (`lendara-shield.duckdns.org, 64-226-124-221.sslip.io, NEW {`). Reload.
2. Server `.env` and rebuild (next section). Verify on `NEW`.
3. Caddy: move `NEW` to its own block and turn the old names into redirects (below). Reload.

## Caddy (`deploy/Caddyfile`, then the server)

- Serve the site on `NEW` (it imports `security_headers`, so HSTS applies to the new name too).
- Point the bare-IP `http://64.226.124.221` block's `redir` at `https://NEW{uri}` too.
- In step 3, turn the old names into redirects that keep the path:

  ```caddyfile
  lendara-shield.duckdns.org, 64-226-124-221.sslip.io {
  	import security_headers
  	redir https://NEW{uri} permanent
  }
  ```

  A redirect keeps links in the README, GitBook, the SCF application and shared messages working. API clients must not rely on it for `POST` requests: point them to `NEW`.
- Apply as AGENTS.md §9 says: back up `/etc/caddy/Caddyfile`, copy, `caddy validate`, `systemctl reload caddy`.

## Configuration on the server (`.env`)

| Variable | New value | Then |
|---|---|---|
| `CORS_ALLOWED_ORIGINS` | `https://NEW` | restart `shield-api` |
| `SHIELD_BOOTNODE_URL` | `https://NEW/bootnode` | restart `shield-api` |
| `NEXT_PUBLIC_API_BASE_URL` | `https://NEW` | **rebuild** the web (`make build-js`), restart `shield-web` |
| `NEXT_PUBLIC_BOOTNODE_URL` | `https://NEW/bootnode` | **rebuild** the web |

`NEXT_PUBLIC_*` values are compiled into the web bundle and into its CSP, so a restart alone does not pick them up.

## What users notice

- **Freighter asks to connect again**: its permissions are per site.
- **The browser starts fresh on `NEW`**: browser storage is per origin. The web derives the user's pool keys again when the wallet connects, and the SDK syncs the notes from the pool (through the bootnode), so the private balance comes back. The first proof downloads the circuits again (~32 MB). Check this with a real user right after the move.
- Nothing changes on-chain: same pool, same notes, same anchor.

## Text to update (one docs PR)

`README.md`, `docs/gitbook/**` (English and Spanish: README, technical architecture, testnet verification), `docs/scf-context.md` and the SCF application. Leave dated reports in `docs/testing/` as they are: they describe what was true on their date.

## Verify

```bash
curl -sI https://NEW/en | grep -i strict-transport          # HSTS on the new name
curl -s  https://NEW/v1/info | jq .pool.max_deposit          # API through the new name
# The bootnode through the new name (JSON-RPC over POST), and its archive:
curl -s https://NEW/bootnode -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' | jq .result.sequence
make test-testnet                                            # on the server: includes the bootnode's archived allow-list events
curl -sI https://lendara-shield.duckdns.org/en/docs | grep -i location   # after step 3: old name redirects with the path
```

Then run a ramp deposit end to end on `NEW`, a private transfer from a browser (it proves against the pool's history through the bootnode), and an onboarding in a browser that used the old name. The bootnode service and its archive (`/home/shield/bootnode`) stay as they are: only the public URL changes.
