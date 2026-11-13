# Pool maximum deposit lost at startup — 2026-11-13

## Symptom

`GET /v1/info` answered `"max_deposit": null` although the pool's instance
storage holds `MaximumDepositAmount = 10000000000` stroops (1,000 USDC). With the
cap unknown, the shielding worker's early over-cap check is off: an over-cap
intent would fail its attempts instead of stopping at once.

## Cause

At 06:05:56 UTC unattended-upgrades started installing glibc
`2.39-0ubuntu8.12`, and `needrestart` then restarted services together, among
them `shield-api`, `systemd-resolved` and `systemd-networkd`, which came back
within about half a second of each other:

| Time (UTC) | Event |
|---|---|
| 06:06:17.897 | `shield-api` started |
| 06:06:17.964 | `could not read the pool's maximum deposit; over-cap intents will fail after retries`, error `RPC unreachable: error sending request for url (https://soroban-rpc.testnet.stellar.gateway.fm/)` |
| 06:06:18.159 | `cannot check the intake (pool keys / allow-list)`, error `RPC unreachable: error sending request for url (https://lendara-shield.duckdns.org/bootnode)` |
| 06:06:18.189 | `systemd-resolved` started |
| 06:06:18.388 | `systemd-networkd` started |

The API read the cap once, with no retry, while the host's resolver and
network service were restarting: the likely cause, given the timing and the
error (failed before any HTTP answer). The cap then stayed unknown until the
next restart. A later probe from the server reached the RPC (`getLatestLedger`:
HTTP 200 in 0.19 s). The intake check failed the same way; it only logs, and
the intake was in fact ready.

## Fix

`SorobanRpc::pool_max_deposit_retrying` retries transport failures after 1, 2, 4
and 8 s, within a 30 s budget that covers requests and waits (the API does not
listen until the read ends). A response body cut short now counts as a
transport failure; a complete body that does not parse is still a decode error,
returned without retrying. Every other consumer maps both variants the same
way, so no other behaviour changes. Tests cover recovery, exhaustion,
permanent errors, a refused connection, a truncated body, the backoff timing and
the total budget (paused clock). AGENTS.md §9 records the pitfall. An
independent review of the diff and of the deployment plan found no remaining
bug; its requested changes were applied.

## Deployment

- **Build:** a fresh clone at `ccbc23b` (the server's API base; `apps/api` is
  identical on `develop`, with no new migrations) plus the `apps/api` part of the
  diff (sha256 `b3868515c3379139e9e1a70fe5d75bd38880f59445e5d2d90595aab0ac158393`),
  built with `cargo build --manifest-path apps/api/Cargo.toml --release --locked`
  in `/home/shield/releases/ccbc23b-maxdeposit-api`. Binary sha256
  `769f1de05b7460a26819adda64c65a55d70a7531119aeb2f7a0935a30cafe819`. The running
  service was untouched while building.
- **Cutover (21:36 UTC):** no intent in `funds_received`, `shielding` or
  `transferring` (7 `awaiting_anchor`, 6 `completed`, unchanged before and after,
  with no `intent_events` in the surrounding two hours); `shield-api` stopped; no
  `pool-cli` process; `.env` and a `pg_dump` backed up with sha256 checksums (server
  only, mode 600). The
  intake wallet and `pool-cli` were not changed.
- **Override:** `/etc/systemd/system/shield-api.service.d/10-maxdeposit-retry.conf`
  clears `ExecStart` and points it at the release binary; `WorkingDirectory` and
  `EnvironmentFile` are unchanged. `systemctl daemon-reload`, then start. Downtime
  about 3 s.
- **Startup log:** `pool maximum deposit read` with `max_deposit: 10000000000`
  and `intake ready: pool keys derived and allow-listed`; no warnings or errors
  afterwards. `/v1/info` answers `"max_deposit":"1000"`.

Rollback: remove the drop-in, `systemctl daemon-reload`, restart. When the
merged commit is deployed from the server's checkout, remove or update the
drop-in the same way (a checkout update alone does not change the running
binary), and keep the release directory until its replacement is verified.

## Verification

`make test-testnet` from the release directory: all seven on-chain checks pass
(balances, trustline, SAC, pool policy, SPP `verify-deployment.sh`, bootnode),
and the ignored `testnet_anchor` (1) and `testnet_asp` (2) tests pass.

Live ramp with a throwaway friendbot account, run without a browser (SEP-53
signatures with `stellar message sign`, pool keys from `pool-cli` in a throwaway
wallet, the sandbox widget API with the approved scenario). Intent
`01a1d6d7-bf90-7243-95d3-791c507474f6`: CRC 1,000 → 1.4607843 USDC, `completed`
at 21:37:58, 52 s after creation. Statuses polled every 5 s: `awaiting_anchor`
21:37:07, `funds_received` 21:37:22, `shielding` 21:37:37, `transferring`
21:37:52, `completed` 21:38:02.

| Step | Transaction |
|---|---|
| Anchor pays Shield's omnibus account (memo) | [`2e7e20c5…`](https://stellar.expert/explorer/testnet/tx/2e7e20c5588704b20cac197e2361fd522365d8a72191ea6d7914d3e5f09bf92d) |
| Shield deposits into the pool, `ext_amount > 0` | [`711dfba2…`](https://stellar.expert/explorer/testnet/tx/711dfba210ab0040020e0ea8bbcb094d899f96d85154f63f4e7f29fc2e3b2527) |
| Private transfer to the user's keys, `ext_amount = 0` | [`9436033d…`](https://stellar.expert/explorer/testnet/tx/9436033d5c73d070f8fea9d88cc719fdf7a5dba760193dff39f6dd1a6d9e6f66) |

All three are successful on Horizon; the deposit and transfer are signed by the
intake (`GC5VXVUO…`). The user's address appears 0 times in their transactions,
operations and effects. The user's own keys read a private balance of
`14607843` stroops. The throwaway key and wallet were shredded.

## Limits

- The retry covers about 15 s of immediate failures (five attempts over the
  1/2/4/8 s waits), or less if requests are slow; 30 s is the upper bound. If the
  RPC is still unreachable then, the cap stays unknown until the next restart
  (the previous fallback, now reached less often).
- The retry was exercised by tests, not by a real network restart.
- `needrestart` can still restart the API together with the network; the API
  recovers by itself if the network is back before the retries run out.
