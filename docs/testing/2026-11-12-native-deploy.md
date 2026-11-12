# Native SDK 0.3.0 deployment and live ramp check — 2026-11-12

## What changed on the demo server

`pool-cli` built from `b0dcffe` (SPP native SDK `=0.3.0`, `libsqlite3-sys 0.39.1` /
SQLite 3.54.1, PR #87) replaced the SDK 0.2.1 build. The API binary, database,
contracts, browser SDK, circuit lock and web were not changed: between the
server's API build (`ccbc23b`) and `b0dcffe`, `apps/api` is unchanged and there
are no new migrations; the server-side change is `apps/pool-cli` (the web fix
from #86 had already been deployed separately). The deployment followed the procedure in
[2026-11-12-native-sqlite.md](2026-11-12-native-sqlite.md); an independent
review of the plan found no path to a double deposit or lost funds and its
requested changes were applied.

## Procedure

1. **Build** in a separate release directory (`cargo build --release --locked`),
   leaving the running binary untouched.
2. **Rehearsal on copies** while the API kept running. Two consistent copies of
   the intake wallet were taken with SQLite's online backup API (integrity `ok`,
   schema v5). Both binaries ran under `env -i` with only the pool-cli variables
   and the copy's path:
   - `keys`: identical account, ASP leaf, note and encryption keys.
   - `balance`: `0` stroops on both (the intake forwards every deposit).
   - The 0.3.0 copy migrated to schema v7.
   - `prepare-deposit --amount 1` on the 0.3.0 copy produced a Groth16 proof in
     16 s; simulation reached the token transfer and stopped at
     `Error(Contract, #10)` because the intake held no public USDC. Nothing was
     submitted.
   - The live wallet's sha256 was unchanged afterwards.
3. **Cutover** with no intent in `funds_received`, `shielding` or `transferring`:
   stop `shield-api`, confirm no `pool-cli` process and no `-journal`/`-wal`/`-shm` sidecars, back
   up the wallet (schema v5), the 0.2.1 binary, `.env` and a `pg_dump` with
   sha256 checksums (server only, mode 600).
4. Only the `POOL_CLI_PATH` line of `.env` changed. With the API stopped, the new
   binary's `keys` and `balance` migrated the live wallet to v7 (integrity `ok`,
   same keys, balance `0`).
5. `shield-api` restarted at 04:01:43 UTC and logged
   `intake ready: pool keys derived and allow-listed`, with no warnings or
   errors afterwards. Downtime was about one minute. The rehearsal copies were
   shredded.

Rollback, if ever needed: stop the API, restore the v5 wallet backup and the
previous `POOL_CLI_PATH`, resync with the 0.2.1 binary and reconcile any
transaction hashes persisted after the cutover before restarting. SDK 0.2.1
cannot open the v7 wallet.

## Live ramp check

A fresh testnet account funded by friendbot (throwaway), run without a browser: SEP-53 signatures with
`stellar message sign`, the user's pool keys derived by `pool-cli` in a
throwaway wallet, and the sandbox anchor's widget API with the approved test
card. Intent `01a1cdeb-bdcd-7681-b547-d2dd4a6c1cf4`: CRC 1,000 → 1.4607843 USDC.

Statuses as observed by polling `GET /v1/intents/{id}` every 5 s, so each lags
the actual transition by a few seconds; on-chain times are ledger close times.

| Time (UTC) | Status |
|---|---|
| 04:02:22 | `awaiting_anchor` (SEP-24 interactive deposit created) |
| 04:02:24 | anchor payment requested (approved card); it closed on-chain at 04:02:28 |
| 04:02:38 | `funds_received` |
| 04:02:56 | `shielding` |
| 04:03:14 | `transferring` |
| 04:03:20 | `completed` (API `completed_at` 04:03:19: 51 s after the anchor's payment landed, 58 s after the intent was created) |

| Step | Transaction |
|---|---|
| Anchor pays Shield's omnibus account (memo) | [`95992ef6…`](https://stellar.expert/explorer/testnet/tx/95992ef66ec8bd04bd8f1a49bc61b8ba3ff5113ef8cce26dd5b401b6ca47d987) |
| Shield deposits into the pool, `ext_amount > 0` | [`d1d967d0…`](https://stellar.expert/explorer/testnet/tx/d1d967d0121f11251b297453883689f1c01aecb02961294924b64e1fb535c69a) |
| Private transfer to the user's keys, `ext_amount = 0` | [`7ef1791a…`](https://stellar.expert/explorer/testnet/tx/7ef1791a434c8c1bc4d287f433f2d423309b3d4c31e51aca41e623c5f874429f) |

All three are successful on Horizon. The user's address appears **0 times** in
their transactions, operations and effects; the deposit and transfer are
signed by the intake (`GC5VXVUO…EHPQ`). The user's own keys then read a private
balance of `14607843` stroops (1.4607843 USDC).
