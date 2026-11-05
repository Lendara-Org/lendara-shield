# Friction Log — Lendara Shield (HackMeridian 2026)

Lendara Shield is a compliant privacy layer for Stellar, built for HackMeridian 2026 (Genesis track). It connects a SEP-24 anchor to a [Stellar Private Payments](https://github.com/NethermindEth/stellar-private-payments) (SPP) pool. In **ramp mode**, the anchor pays USDC to Shield's intake account, Shield deposits it into the pool and transfers it privately to the user's pool keys, so **the user's address never appears on-chain**. Shield also acts as the pool's ASP (allow-list and block-list), and holds the Global View Key for audits.

We built most of it on Oct 25–26, 2026: a Rust API (Axum + Postgres), a separate `pool-cli` prover, a Next.js web with Freighter, and an audit panel. We verified it again on testnet on Oct 28. We worked with Claude Code as our AI assistant.

> Time costs below are rough estimates, reconstructed from our commit timestamps and notes.

## Stellar tools we used

| Tool | How we used it |
|---|---|
| **Stellar Private Payments `sdk-v0.2.1`** (Nethermind): Soroban contracts, Rust crate `stellar-private-payments`, npm package | We deployed our own `allowlist-blocklist` + `gvk-traceable` pool for USDC with `deploy.sh`. The Rust crate runs server-side deposits, private transfers to raw keys, and GVK audits. The browser SDK derives the user's keys and ASP leaf, and runs direct deposits, transfers and withdrawals. |
| **Soroban contracts and RPC** | We built our own small JSON-RPC client (`stellar-xdr` + `ed25519-dalek`) to insert leaves into the ASP contracts, and we read `LeafAdded` events to check membership. |
| **Stellar CLI** | Idempotent testnet setup script: `stellar keys generate/fund`, `stellar tx new change-trust`, `stellar contract id asset` for the USDC SAC, `stellar contract invoke`. We also used `stellar message sign` to cross-check SEP-53 signatures. |
| **Freighter** | Wallet connection, and SEP-53 `signMessage` to prove account ownership at onboarding and to sign each ramp request. |
| **Horizon** | Watcher for the anchor's payments to the intake (matched by memo), and resolving the audit's ledgers to transaction hashes. |
| **SEP-1, SEP-10, SEP-24, SEP-53** | Shield is a SEP-24 *client* of an anchor (our own Lendara sandbox). We wrote the `stellar.toml` parser, full SEP-10 challenge verification, and the interactive deposit to the intake with an `id` memo by hand in Rust. |
| **Stellar MCPs (Raven or others)** | Not used. |

## Friction points

### 1. The SPP Rust SDK and `sqlx` cannot live in the same binary
- **What happened:** both pull `libsqlite3-sys` with incompatible versions (SPP keeps its wallet in SQLite; our API uses `sqlx` for Postgres), so the API would not build with the SDK in it.
- **Cost:** ~1–2 h, including redesigning the architecture.
- **Workaround:** a separate stateless `pool-cli` crate that speaks JSON over stdin/stdout, run by the API as a subprocess. It works, but it adds a process boundary, error mapping and a lock, because the worker and the auditor share the intake's SQLite wallet.

### 2. Byte order of pool keys is not documented
- **What happened:** the SDK prints the ASP leaf big-endian but the note public key little-endian. Our block-list key and the GVK audit owners only matched once we reversed the bytes.
- **Cost:** ~1 h, found only by testing against testnet.
- **Workaround:** a `field_le` helper, and validation of every field element against BN254 in the right order.

### 3. A new pool silently needs the intake account initialised
- **What happened:** a fresh allow-list does not contain the intake's leaf, and a fresh wallet has no keys. Every shielding attempt failed (`no privacy keys found` or `not_allowed … RegisterAtASP`), and funded intents ended in `error` with the USDC stuck in the intake.
- **Cost:** ~2 h on hackathon morning.
- **Workaround:** `make init-intake` and a check at startup. Because the allow-list contract only stores its root, we read membership from its `LeafAdded` events since the deployment ledger.

### 4. The browser prover times out after 30 s, and the first proof includes a ~32 MB download
- **What happened:** `PROVE_TIMEOUT_MS` is compiled into the SDK's WASM worker and can't be changed from JS. On a slow connection, a browser's first proof timed out while it was still downloading the circuit.
- **Cost:** ~1 h.
- **Workaround:** preload the circuits into the SDK's own Cache Storage when the wallet opens, and retry once only if proving timed out with nothing sent.

### 5. The public RPC keeps only ~7 days of events, and proving needs the pool's full history
- **What happened:** a pool stops working for proofs about a week after deployment unless you run a bootnode. Our testnet pool left the window around Nov 2, so the live demo stops proving after that.
- **Cost:** ~30 min to understand during the spike; it is an ongoing cost for any long-lived demo.
- **Workaround:** we deployed late enough to cover Meridian. Long term: run SPP's `tools/bootnode` or redeploy.

### 6. No ASP admin calls in the SDK
- **What happened:** inserting and deleting allow-list and block-list leaves is a plain contract invocation by the pool admin, and the SDK has no helper for it. We also learned that the allow-list is append-only, so blocking means writing to the block-list.
- **Cost:** ~1.5 h (our own Soroban client, signing and sequence handling).
- **Workaround:** a `SorobanAspContract` adapter on raw JSON-RPC.

### 7. SDK pool transactions have no time bounds
- **What happened:** `valid_until` is null, so a transaction the RPC reports as `not_found` might still land later. Resubmitting it naively could double-spend the intake's intent.
- **Cost:** ~1 h of design for an idempotent worker.
- **Workaround:** persist the signed envelope before submitting. Replace it only once the intake's sequence has passed it **and** it has stayed unseen for 60 s.

### 8. Block-list writes invalidate proofs built before them
- **What happened:** any ASP update changes the root, so in-flight proofs fail and the wallet can report it needs a resync. At first our worker counted an RPC lag as a failed attempt.
- **Cost:** ~45 min.
- **Workaround:** treat `sync_required` as retryable without spending an attempt (wait 5 s), and re-prove after each block-list write.

### 9. The browser SDK reports failures as English free text
- **What happened:** a declined signature, a short balance and token errors such as `Error(Contract, #13)` (missing trustline) all come back as messages, not codes. We had to pattern-match strings to show useful, translated errors.
- **Cost:** ~1 h.
- **Workaround:** `lib/pool/errors.ts`, with tests, which will break if an SDK upgrade changes the texts.

### 10. A missing USDC trustline only shows up at simulation time
- **What happened:** a new account without a trustline got a generic SAC error (`#13`) only after starting a direct deposit.
- **Cost:** ~1 h during the Oct 28 verification.
- **Workaround:** read the trustline from the RPC before the form, and block it with instructions that name the exact USDC issuer the pool accepts.

### 11. Packaging and versions
- The SPP browser SDK (JS, WASM, workers, circuits) can't be bundled by Next.js; we copy it into `public/` with a script.
- npm's `alpha` dist-tag points to a stale `0.1.0-alpha.1`; the right one is `latest` (0.2.1). Contracts, crate and npm must be pinned to the same tag.
- The `pool-cli` debug build proves very slowly, so it has to run in release.
- **Cost:** ~1 h in total.

### 12. Privacy pitfalls that are easy to miss
- Registering in SPP's public key registry is a public transaction from the user's account, so it reveals that the address uses the pool. We transfer to raw keys instead.
- The source account of a pool transaction is visible, so in direct mode the user's address is exposed until there is a relayer.
- `deploy.sh` warns when the asset issuer can revoke or claw back, which would let it freeze the whole pool.
- **Cost:** ~1 h of reading and design. These were documented in the code, but not up front.

### 13. Smaller things
- SEP-10 `web_auth_domain` vs. `home_domain` when `stellar.toml` and the auth endpoint live on different hosts: ~20 min.
- `STELLAR_NETWORK_PASSPHRASE` contains spaces and `;`, so `source .env` breaks in bash: ~10 min.

## What worked well
- **SPP's Rust SDK** is well structured: the `prepare → prove → sign → submit → confirm` split made an idempotent worker possible, and transfers to raw keys let us keep users off the registry.
- **`deploy.sh`** with committed verifying keys for every policy combination: deploying our own pool for USDC was quick.
- **Stellar CLI** made the testnet setup scriptable and idempotent.
- **Freighter's SEP-53 `signMessage`** gave us a simple proof of account ownership, which we could verify server-side.
- **SEP-10 and SEP-24** let Shield work with an anchor without changing the anchor: the memo on the deposit was all we needed.
- **Testnet and friendbot** were fast and reliable during the whole hackathon.

## What would have made it easier
1. Documented byte order (and a single canonical encoding) for SPP keys and leaves.
2. ASP admin helpers in the SDK (insert/delete leaf, membership check) and a "bootstrap a new pool account" guide.
3. Structured error codes in the browser SDK instead of English messages.
4. A configurable prover timeout, or a documented way to preload circuits.
5. Time bounds on SDK transactions, or documented guidance for safe resubmission.
6. A hosted indexer/bootnode for testnet pools, or a clear warning about the 7-day RPC window in the quickstart.
7. A Rust SDK that doesn't pin `libsqlite3-sys` in a way that conflicts with `sqlx`, or a storage-agnostic wallet.
8. An end-to-end example of a privacy pool behind a SEP-24 anchor.
9. A Stellar-maintained Rust helper for SEP-10 client-side challenge verification.
