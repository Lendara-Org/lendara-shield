# Slow shielding on testnet: the public RPC's latency (2026-11-06)

**Summary.** A ramp deposit sometimes took up to 15 minutes to reach the user's private balance. The measured cause was the latency of the public SDF testnet RPC (`https://soroban-testnet.stellar.org`), which answered each request in 0.3 to 10+ seconds that day. Every proof needs about nine sequential RPC requests, so one attempt took about 42 seconds. In the middle of each attempt, between the wallet's sync and its read of the ASP state, the chain often closed new ledgers, and the SDK then asked for a resync and a retry. With a fast RPC (Gateway.fm's public testnet endpoint, about 0.1 s per request), the same preparation took 14–17 seconds and passed on the first attempt every time.

Neither the Stellar network nor the SPP SDK was broken: the chain closed ledgers every ~5 seconds and confirmed all of our transactions throughout, and the SDK's refusal is intended behaviour. A shared, free, public endpoint was slow, and Shield multiplies any per-request latency by nine.

## What we saw

Live demo, intent `01a1b219-cbc9-7293-90ac-5f8a101cc495` (sandbox `delayed` scenario):

| UTC | Event |
|---|---|
| 18:23:55 | The anchor's payment confirmed → `funds_received` |
| 18:24:21 → 18:28:15 | 8 × `ASP membership sync required: SyncRequired(Some(1))` (no allow-list change on-chain in that window) |
| 18:28:43 | Pool deposit prepared (`6f1e9a12…`) |
| 18:29:46 → 18:37:05 | 10 more `SyncRequired` waits |
| 18:38:31 | `completed`, with the full private balance. 15 minutes end to end, 0 attempts spent |

An earlier run of the same flow took about 3 minutes. Funds were never at risk: the worker treats `SyncRequired` as a wait, not a failed attempt (AGENTS.md §9), so the intent could not end in `error`. It just looked stuck.

## Why the SDK says `SyncRequired`

`pool-cli` prepares each pool transaction with the SDK's `prove_next`. Every call:

1. **Syncs** the wallet's local index from the RPC's events (`getEvents`, `getLatestLedger`, plus a probe), up to some ledger *N*.
2. **Reads the pool's and the ASP's contract state** (`getLedgerEntries`, and a `simulateTransaction` against the block-list). Those reads report the RPC's current ledger, say *N + 2*.
3. **Checks the membership precondition** (`check_asp_membership_precondition`, `state/storage.rs` in `stellar-private-payments` 0.2.1). If the state read's ledger is ahead of the indexed ledger, it returns `SyncRequired(gap)`. It does this before comparing the allow-list root.

So `SyncRequired(Some(1))` does not mean "one user is missing". It means the chain moved past the wallet's index between steps 1 and 2. That is expected and retryable. The SDK's own high-level executor, its web SDK and its CLI all retry it in-process, up to 50 times every 200 ms. Shield calls the lower-level `prove_next` on purpose, so the API can persist each transaction hash before submitting (AGENTS.md §5.3), and hands each refusal back to the worker.

Whether an attempt passes is therefore a race between the ledger close time (~5 s) and the interval from the end of the sync (step 1) to the ASP state read (step 2). Proving, simulation and signing come later and do not affect the check. That interval, and the cost of every retry, are set by the RPC's latency.

## Measurements

All runs prepare a private transfer from a test wallet with `pool-cli prepare-transfer` (proved, simulated and signed locally, **never submitted**), using a build with per-attempt tracing (branch `fix/api-pool-cli-sync-retry`, `SHIELD_SYNC_TRACE_FILE`) and an in-process retry of up to 10 attempts.

**1. Public SDF RPC, 8 runs.**

| | |
|---|---|
| Time per preparation | 76, 125, 147, 173, 185, 190, 275 and 510 s |
| `prove_next` attempts | 26: 8 ok (~31 %), 18 `SyncRequired`; 3 of 8 runs passed on the first try |
| Duration of one attempt | ~42 s, the same whether it passed or was refused |
| `gap` | 1–5 ledgers (mostly 2–3), never growing |

An attempt that passed and one that was refused cost the same, and the wallet never fell further behind. That is a race, not a backlog.

**2. Where the 42 s went.** A logging proxy between `pool-cli` and the RPC recorded every request (method, duration, size; never payloads). Over 34 requests:

| Method | Requests | Average | Max |
|---|---|---|---|
| `getEvents` | 18 | 4.7 s | 10.9 s |
| `getLatestLedger` | 7 | 4.6 s | 10.8 s |
| `getLedgerEntries` | 6 | 2.5 s | 12.1 s |
| `simulateTransaction` | 3 | 4.7 s | 9.5 s |

Even `getLatestLedger`, the cheapest call, averaged 4.6 s. Measured directly with `curl` (no proxy), from a developer machine and from the demo server, the same call took 0.3 to 11 s. The latency was the endpoint's, not the client's.

**3. The four public testnet RPCs listed in the [Stellar docs](https://developers.stellar.org/docs/data/apis/rpc/providers), from the demo server:**

| Endpoint | `getLatestLedger` | `getEvents` (allow-list) | Notes |
|---|---|---|---|
| `soroban-testnet.stellar.org` (SDF) | 0.3–10.7 s | 8.7 s | healthy, erratic |
| `soroban-rpc.testnet.stellar.gateway.fm` | 0.13–0.22 s | 0.11 s, same 7 events | healthy, same retention window |
| `stellar-soroban-testnet-public.nodies.app` | ~0.1 s | did not answer `getEvents` as expected | not usable as tested |
| `stellar.liquify.com/…/testnet` | 12 s | — | 503 at the time |

**4. Gateway.fm, 8 runs, same build and wallet:**

| | Public SDF RPC | Gateway.fm |
|---|---|---|
| Time per preparation | 76–510 s | **14–17 s** |
| Passed on the first attempt | 3 / 8 | **8 / 8** |
| `SyncRequired` refusals | 18 | **0** |
| Duration of one attempt | ~42 s | **~5 s** |

With ~0.1 s requests, the sync and the state read happen within a fraction of a second, so they rarely straddle a ledger close, and the precondition passed every time in this sample. A fast RPC makes the race unlikely, not impossible: a refusal is still retried as before.

**5. Live ramp run on the demo after switching to Gateway.fm** (fresh test account, sandbox card approved, ~21:14 UTC). Times from Horizon, independently checked by Codex:

| Interval | Before (SDF RPC, 18:23–18:38) | After (Gateway.fm, 21:14) |
|---|---|---|
| Anchor payment → pool deposit | 300 s | 20 s |
| Pool deposit → private transfer | 570 s | 20 s |
| **Anchor payment → private transfer** | **870 s** | **40 s** |
| `SyncRequired` waits in the API log | 18 | 0 |

Transactions after the switch: `7b93d63f…` (anchor → intake), `2d1f020b…` (pool deposit, `ext_amount` = the anchor's 1.4607843 USDC), `2e45d1d9…` (private transfer, `ext_amount` = 0). The user's address appears in none of them. One run on each side, so it shows the size of the change, not a distribution.

## Conclusions

- **Cause:** the public SDF testnet RPC was slow (seconds per request, erratic). Shield's shielding path makes about nine sequential requests per proof, so the slowness became ~42-second attempts that lost the race against the ledger close time most of the time.
- **Ruled out by the evidence:** the Stellar network (it kept closing ledgers and confirming our transactions), and the cost of starting a fresh `pool-cli` process (opening the SDK client takes ~7 s).
- **Not measured, so not ruled out for the original incident:** contention on the wallet lock shared by the shielding worker and the auditor (the standalone `pool-cli` runs above bypass it; one 13 s audit request ran before the waits began), and bootnode delays. The SDK's membership check is intended upstream (its `main` branch has the same check), but checking the ASP root before the ledger gap would refuse less often, so the SDK's design also shapes how often the race is lost.
- **Fix:** point `SOROBAN_RPC_URL` (API and `pool-cli`) and `NEXT_PUBLIC_SOROBAN_RPC_URL` (browser) at a fast, healthy RPC that holds the pool's history. Both are configuration, and the web's CSP follows the variable. The bootnode's upstream RPC is configured separately (`BOOTNODE_UPSTREAM_RPC_URL`, otherwise the manifest's `rpcUrl`) and does not follow these variables. A self-hosted `stellar-rpc` would remove the third-party dependency and could keep more than 7 days of history; it is on the roadmap as H11 (AGENTS.md §10).
- **Not adopted:** retrying `SyncRequired` inside one `pool-cli` run (the experimental branch). With a slow RPC it did not break the race, and 10 attempts × 42 s exceeds the API's 180 s budget for a `pool-cli` run. A timeout counts as a failed attempt and could send a funded intent to `error`, which `SyncRequired` never does. With a fast RPC it is not needed.

## Reproduce

- Per-attempt trace: build `pool-cli` from `fix/api-pool-cli-sync-retry` and set `SHIELD_SYNC_TRACE_FILE=/path/trace.jsonl`. Each line has the event (`open` or `prove_next`), its duration, the outcome and `gap`, and the RPC's ledger right after. No keys, notes or amounts.
- Independent check (Codex, ~21:06 UTC): Gateway.fm and SDF returned the same network passphrase, the same ledger hash at an overlapping ledger, the same 7 allow-list events and the same contract ledger entry, and Gateway.fm answers browser CORS preflights (`Access-Control-Allow-Origin: *`).
- RPC latency: `curl -w '%{time_total}\n' -X POST <rpc> -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}'`, a few times in a row. Fast endpoints answer in about 0.1–0.3 s; anything in seconds will slow shielding down.
