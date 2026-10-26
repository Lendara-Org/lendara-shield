# Security

> Security posture of Lendara Shield as it is today, not a generic checklist. It complements the [threat model](docs/threat-model.md), the [scenarios](docs/scenarios.md) and the rules in [AGENTS.md](AGENTS.md) §5.
>
> Three kinds of statement, kept apart:
> - **Fact:** verified in the code, the configuration or a command run on this revision. Sections 1–7 state facts unless marked otherwise.
> - **Hypothesis:** plausible from reading the code, not yet confirmed by a test or a run. The first review's hypotheses are all verified now (§7.3); none is open.
> - **Recommendation:** a suggested change, not implemented. Only in §8.
>
> Passing tests do not make Shield secure. They show that the behaviours they cover hold; they say nothing about what they do not cover.

---

## 1. Status and scope

- **Testnet only.** The API refuses a deployment for any other network (`apps/api/src/deployment.rs`, `TESTNET`) and so does `pool-cli` (`load_config` in `apps/pool-cli/src/pool.rs`). Both hard-code the testnet passphrase. *(Fact)*
- **Unaudited building blocks.** The privacy pool is Stellar Private Payments (SPP) `sdk-v0.2.1`, a developer preview whose contracts have no audit. Shield itself has had no security review. *(Fact)*
- **Hackathon build.** No production hosting, key management (HSM/KMS) or monitoring exists, and the rate limit is a per-IP, in-memory one (see [threat model, out of scope](docs/threat-model.md#out-of-scope-for-the-hackathon)). *(Fact)*
- **In scope of this document:** `apps/api`, `apps/pool-cli`, `apps/web`, `packages/api-types`, `scripts/`, the CI workflows and Shield's use of SPP, Soroban, Horizon and SEP-1/10/24 anchors.
- **Out of scope:** the SPP contracts and SDK themselves (Nethermind), the anchor (`Lendara-Org/Lendara-monorepo`, never modified from here), Freighter and the Stellar network.

**Do not use Shield with real funds.**

## 2. Architecture and trust boundaries

```
 User's browser ──────────────── HTTPS (CORS allow-list) ──────────────► Shield API (Rust/Axum)
 Freighter + SPP browser SDK                                              │  ├─► Postgres (sqlx)
 (spending keys, proofs in direct mode)                                   │  ├─► Soroban RPC   (ASP writes, signed by the operator)
        │                                                                 │  ├─► Horizon       (intake payments, public explorer view)
        │ widget (popup)                                                  │  ├─► Anchor        (SEP-1/10/24, signed by the intake)
        ▼                                                                 │  └─► pool-cli      (subprocess: prover, intake wallet, GVK audit)
 SEP-24 anchor ──── USDC + id memo ────► intake (omnibus) account ──► SPP pool (Soroban)
```

| Boundary | What crosses it | Who is trusted |
|---|---|---|
| Browser → API | Public pool keys, ASP leaf, SEP-53 signature, intents, the auditor token | Neither side trusts the other: the API validates every input with newtypes; the browser never sends spending keys |
| API → `pool-cli` | Command-line arguments (no secrets) and an environment rebuilt from scratch | `pool-cli` gets only the variables in `FORWARDED_ENV`; only the auditor's instance gets `AUDITOR_ENV` (`apps/api/src/adapters/pool_cli.rs`, `env_clear()`) |
| API → anchor | SEP-10 challenge signatures and SEP-24 requests made with the intake key | The anchor is trusted to do KYC and to report its own payment (`stellar_transaction_id`); its `stellar.toml`, challenge and URLs are checked first (§5.1) |
| API → Soroban RPC / Horizon | Signed transactions; reads of status, sequence and payments | Trusted for availability and for what they report. Shield cannot tell whether a node lies, and it does not cross-check against a second node |
| API → Postgres | The user ↔ key relation, intents, signed envelopes | Trusted. The database holds what logs must not (AGENTS.md §5.2) |
| Network → everyone | The pool contracts, the asset's SAC and its issuer | Trusted as deployed (`docs/spikes/spp.md`); see §7 limit L5 on the issuer |

## 3. Assets, actors and attack surface

### 3.1 Assets

| Asset | Where it lives | Protection today |
|---|---|---|
| Users' spending keys | The user's browser only (SPP SDK storage, derived from a Freighter signature) | Never sent to Shield; `ShieldWallet` exposes only public keys and the ASP leaf (`apps/web/src/lib/pool/types.ts`) |
| Operator key (pool admin + ASP admin) | `SHIELD_OPERATOR_SECRET` in the API's environment | `secrecy::SecretString`; never logged (`apps/api/src/config.rs`) |
| Intake (omnibus) key and its SPP wallet | `SHIELD_INTAKE_SECRET`; SQLite wallet at `SHIELD_INTAKE_WALLET_PATH`, outside the repo | `SecretString`; passed to `pool-cli` through its environment, never as an argument |
| Global View Key (auditor) | JSON file at `SHIELD_GVK_KEY_FILE`, outside the repo | Only the auditor's `pool-cli` instance gets its path; never the shielding worker, the browser or logs |
| Funds in custody | The intake account and Shield's intermediate note, between the anchor's payment and the private transfer | Idempotent worker (§4); repeated failures end in `error` for manual review |
| The user ↔ key ↔ account relation | `shield_users` in Postgres | Logs carry ids and route templates only (§5.6) |
| Admin and auditor tokens | `ADMIN_TOKEN`, `AUDITOR_TOKEN` | At least 32 characters (`parse_token` in `config.rs`); SHA-256 digests compared in constant time, so neither content nor length leaks (`constant_time_eq` in `apps/api/src/http/extract.rs`) |
| The anchor's widget session | SEP-24 interactive URL with the anchor's one-time token (verified in the sandbox's source: `NewInteractiveToken`, 15 minutes) | Returned only by the signed `POST /v1/intents` that created the intent; `GET /v1/intents/{id}` answers `null` |

### 3.2 Actors

| Actor | Capability assumed |
|---|---|
| Anonymous Internet client | Calls every public endpoint with arbitrary input, at any rate |
| Another Shield user | Holds a valid onboarding and pool keys of their own |
| Public observer | Reads everything on-chain (explorer, RPC, Horizon), correlates amounts and timing |
| Anyone on Stellar | Sends any asset, with any memo, to the intake account |
| Malicious or compromised anchor | Serves its own `stellar.toml`, challenges, URLs and statuses |
| Compromised server or database | Reads environment, files and tables |
| Malicious dependency or build step | Runs code in CI, the API, `pool-cli` or the browser bundle |

### 3.3 Attack surface

| Surface | Authentication | Source |
|---|---|---|
| `GET /health`, `GET /v1/info` | None | `apps/api/src/http/mod.rs` |
| `POST /v1/onboarding` | SEP-53 signature by the account over its keys; per-IP rate limit | `apps/api/src/asp/ownership.rs`, `apps/api/src/http/rate_limit.rs` |
| `GET /v1/users/{account}` | **None** (limit L9) | `apps/api/src/http/users.rs` |
| `POST /v1/intents` | SEP-53 signature by the account over currency, amount, a single-use `request_id` and `issued_at` (≤ 300 s old); per-IP rate limit | `apps/api/src/http/intents.rs`, `intent_message` in `asp/ownership.rs` |
| `GET /v1/intents/{id}` | **None** (the random intent id); returns no account and no interactive URL | `apps/api/src/http/intents.rs` |
| `POST /v1/asp/block`, `/unblock` | `ADMIN_TOKEN` bearer; disabled if unset | `AdminAuth` in `http/extract.rs` |
| `GET /v1/audit/transactions` | `AUDITOR_TOKEN` bearer; disabled if unset | `AuditorAuth` in `http/extract.rs` |
| `GET /v1/audit/public/{tx_hash}` | None (public data) | `apps/api/src/http/audit.rs` |
| Payments to the intake account | None: anyone can pay it | `apps/api/src/intent/watcher.rs` |
| Anchor responses | Checked (§5.1) | `apps/api/src/adapters/anchor/` |
| The web app and `/spp` assets | Served as static files | `apps/web/`, `scripts/stage-web-sdk.sh` |

## 4. Security properties Shield must preserve

These are the invariants a change must not break. "Enforced by" names where each one is implemented and tested today.

| # | Property | Enforced by |
|---|---|---|
| P1 | In ramp mode, the user's address appears in no on-chain transaction | The deposit goes to the intake (`StartDeposit`); the transfer goes to raw keys (`prepare_transfer`), never through SPP's public key registry (`docs/spikes/spp.md`, finding 8). Verified on testnet with the hashes in the README |
| P2 | Only the anchor's own payment funds an intent | `intent/matching.rs` (`classify`: asset code **and** issuer, `id` memo, status); `intent/anchor_sync.rs` funds only with the payment the anchor reports (`lock_confirmed` in `store/intake_payments.rs`). Random 63-bit memos (migration `0006`) |
| P3 | An intent is deposited into the pool and transferred **exactly once** | `intent/shielding.rs`: prepare → store → submit → status; replace only when the old transaction provably cannot land (`UNSEEN_GRACE`). `tests/shielding_worker.rs` against `FakePool` (`src/testing.rs`), which enforces the sequence rule |
| P4 | Once funds are in custody, an intent ends only in `completed` or `error` | `check` in `apps/api/src/intent/state.rs`; `transition` in `store/intents.rs` (`UPDATE … WHERE status = from` + `intent_events` in one transaction); `CHECK` constraints in migrations `0003` and `0005` |
| P5 | Nobody can attach their pool keys to someone else's account | SEP-53 signature over `onboarding_message` (`asp/ownership.rs`), mirrored byte for byte by `onboardingMessage` (`apps/web/src/lib/onboarding.ts`) |
| P6 | Only allow-listed, non-blocked keys receive new intents | `IntentService::create` requires `AspStatus::Allowed`; the pool also refuses proofs without allow-list membership and block-list non-membership |
| P7 | The three keys stay separate, and the GVK reaches only the auditor process | `FORWARDED_ENV` / `AUDITOR_ENV` in `adapters/pool_cli.rs`. Separation of operator and intake is a **policy only**: nothing checks it at startup (§7, R6) |
| P8 | Logs never pair a user's address with an amount, and never contain keys or notes | HTTP spans use `MatchedPath` (route templates) in `http/mod.rs`; services log intent and user ids |
| P9 | Shield never signs a transaction it did not mean to sign | SEP-10 challenges are verified before signing (`verify` in `adapters/anchor/sep10.rs`); ASP and pool transactions are built and simulated by Shield itself |
| P10 | Only the account's owner starts a deposit in its name, once per signed request | SEP-53 signature over `intent_message` (`asp/ownership.rs`), mirrored by `intentMessage` (`apps/web/src/lib/intent-request.ts`); freshness in `check_fresh` (`http/intents.rs`); `request_id` `UNIQUE` (migration `0007`) |
| P11 | A blocked key gets no new deposit and no transfer | `finalize` in `asp/service.rs` rejects the user's unfunded intents in the same transaction that records the block; `IntentService::create` re-checks the user under the same row lock, and answers `409 user_not_allowed` if the block lands while the anchor is answering; the shielding worker stops before the deposit, before the transfer and before rebuilding a transfer that can no longer land (`stop_for_blocked_user` in `intent/shielding.rs`) |
| P12 | No intent waits forever for an anchor payment Shield cannot confirm | `ANCHOR_PAYMENT_GRACE` (15 min) in `intent/anchor_sync.rs`, then `error` for review (migration `0008`) |

## 5. Stellar-specific considerations

The [Stellar dev skill](https://github.com/stellar/stellar-dev-skill) (revision `e20f352`) informs this section. Its contract-authoring rules (`require_auth`, reinitialization, typed storage keys, TTLs) do not apply here, because Shield deploys no contracts of its own: it deploys SPP's at a pinned tag. What does apply is its client-side checklist, its ZK pitfalls, its asset rules and its data-access notes.

### 5.1 Anchors (SEP-1, SEP-10, SEP-24)

- **Fact:** `stellar.toml` is fetched from `https://<home_domain>/.well-known/stellar.toml`. Its `NETWORK_PASSPHRASE` must match. `WEB_AUTH_ENDPOINT`, `TRANSFER_SERVER_SEP0024` and the interactive URL must be `https` (`secure_url` in `adapters/anchor/client.rs`; plain `http` only for local hosts in tests).
- **Fact:** a SEP-10 challenge is refused unless all of these hold: v1 envelope, source is the anchor's `SIGNING_KEY`, sequence 0, valid time bounds (±`CLOCK_SKEW_SECS`), expected `id` memo, first operation `<home_domain> auth` from the intake, 64-byte nonce, matching `web_auth_domain`, no unexpected operations, and the anchor's signature (`ChallengeError` in `adapters/anchor/sep10.rs`). A test covers each violation (`refuses_every_spec_violation`).
- **Fact:** every anchor HTTP call has a 15 s timeout (`REQUEST_TIMEOUT` in `adapters/anchor/client.rs`).
- **Fact:** the sandbox has no webhooks; statuses are polled every `CHECK_INTERVAL` (20 s).
- **Trust assumption:** the anchor knows the user and the amount (threat model limit 2). A compromised anchor could report a `stellar_transaction_id` for a payment it did not make; it cannot invent the payment itself, because the watcher must have seen it on Horizon with the intent's memo, asset code and issuer.

### 5.2 Soroban transactions and the ASP

- **Fact:** ASP writes (`insert_leaf`, `delete_leaf`) are built, simulated on the RPC and signed by the operator, with a 60 s upper time bound (`DEFAULT_VALIDITY` in `adapters/soroban/asp.rs`). A transaction that is still unknown after that bound is marked failed and replaced (`asp/service.rs`). This matches the skill's "transactions simulated before submission" rule.
- **Fact:** the allow-list is append-only. Revoking a user means inserting their note key into the block-list (`docs/spikes/spp.md`, finding 1). The byte-order rules in AGENTS.md §9 apply: the block-list key is the note key reversed.
- **Fact:** every block-list write invalidates proofs built before it; the worker re-proves (AGENTS.md §9).
- **Fact:** ASP routes (`/v1/onboarding`, `/v1/asp/*`) have their own HTTP timeout, `ASP_REQUEST_TIMEOUT` (105 s) in `http/mod.rs`. It covers the service's whole-request budget for the chain (`CONFIRM_TIMEOUT`, 30 s in total, not per step), the four RPC calls one step may still be making (15 s each) and 15 s for the database. The rest of the API keeps 15 s. A unit test checks the relation.
- **Fact:** the network passphrase is a constant (`TESTNET_PASSPHRASE` in `apps/api/src/main.rs`, `http/info.rs` and `apps/pool-cli/src/pool.rs`). `STELLAR_NETWORK_PASSPHRASE` in `.env.example` is not read by the API's `config.rs`.
- **Fact:** `SOROBAN_RPC_URL` and `HORIZON_URL` must be `https`; plain `http` is accepted only for `localhost` and loopback addresses (`parse_url` in `config.rs`).
- **Fact:** the browser refuses to continue if Freighter is on another network (`connectFreighter` in `apps/web/src/lib/pool/freighter-signer.ts`), as the skill's client-side checklist asks.

### 5.3 SPP pool, proofs and the intake

- **Fact:** SDK pool transactions have **no time bounds** (`valid_until: null`). A transaction that is `not_found` is replaced only after the intake's sequence passed it **and** it stayed unseen for `UNSEEN_GRACE` (60 s) (`intent/shielding.rs`). This covers RPC indexing lag.
- **Fact:** `pool-cli submit` recomputes the envelope's hash and fails if the network reports a different one (`apps/pool-cli/src/pool.rs`).
- **Fact:** the circuit lock is pinned at build time on both sides: `SHIELD_CIRCUITS_LOCK_FILE` for `pool-cli`, and `circuits.json` imported by `apps/web/src/lib/pool/sdk.ts`. Artifacts fetched at runtime are checked against it.
- **Fact:** proof verification and the statement checks (Merkle roots, nullifiers, `ext_amount`) live in SPP's contracts, not in Shield. The skill's ZK pitfalls (verify the statement, not only the proof; anti-replay binding; trusted-setup hygiene) are therefore **SPP's** properties. Shield inherits them without having verified them.
- **Fact:** proving needs the pool's full event history, and the public RPC keeps about 7 days (skill: `data/SKILL.md`). Past that window, run SPP's bootnode or redeploy (threat model limit 8).
- **Fact:** the worker and the auditor share one mutex around `pool-cli`, because both open the intake's SQLite wallet (`sharing_wallet` in `adapters/pool_cli.rs`). `pool-cli` runs with `kill_on_drop` and a 180 s timeout (`DEFAULT_TIMEOUT`).
- **Fact:** the shielding worker runs in its own loop, next to the loop that reads payments and anchor statuses (`background.rs`), so a slow proof delays neither. Shutdown drops a round in progress instead of waiting for it; every step stores its state before any network call.

### 5.4 Asset and issuer

- **Fact:** a payment is matched by asset code **and** issuer (`classify` in `intent/matching.rs`), as the skill's asset rules ask ("validate asset issuer, not just code").
- **Fact:** the sandbox USDC issuer is not `AUTH_IMMUTABLE`, so it could later enable `AUTH_REVOCABLE` or `AUTH_CLAWBACK_ENABLED` and freeze or claw back the pool's balance (threat model limit 5). Shield does not monitor these flags.

### 5.5 Horizon watcher

- **Fact:** each page of payments is applied in one database transaction together with its cursor, and payments are keyed by operation id, so re-reading a page is a no-op (`intent/watcher.rs`). This is the skill's "persist the cursor, make the handler idempotent" rule.
- **Fact:** a payment that matches nothing is stored in `intake_payments` with a reason (`wrong asset`, `no id memo`, …). It is money in custody for manual review and is never refunded or used automatically.

### 5.6 Backend

- **Fact:** both crates set `unsafe_code = "forbid"` and deny `unwrap`, `expect`, `panic`, `dbg` and `todo` outside tests in their `[lints]` (`apps/api/Cargo.toml`, `apps/pool-cli/Cargo.toml`). The API also denies `print_stdout`; `pool-cli` does not, because stdout carries its JSON protocol.
- **Fact:** inputs are parsed into validated newtypes (`apps/api/src/domain/`). Amounts are stroops or decimal strings, never floats.
- **Fact:** internal errors are returned without their cause (`apps/api/src/error.rs`, test `internal_hides_the_source`).
- **Fact:** HTTP requests time out after 15 s (`REQUEST_TIMEOUT` in `http/mod.rs`). CORS allows only `CORS_ALLOWED_ORIGINS`, `GET`/`POST` and the `Content-Type` and `Authorization` headers.
- **Fact:** `POST /v1/onboarding` and `POST /v1/intents` share a per-client-IP fixed-window limit (`RATE_LIMIT_PER_MINUTE`, default 20, `0` disables it), answered with `429 rate_limited` and `Retry-After` (`http/rate_limit.rs`). The client is the TCP peer; `X-Forwarded-For` is not trusted.
- **Fact:** the API refuses to start if `SHIELD_OPERATOR_SECRET` and `SHIELD_INTAKE_SECRET` are the same key (`config.rs`).

### 5.7 Database

- **Fact:** statuses are `text` + `CHECK`; the intent's hashes are `CHECK`ed as 64 lowercase hex characters and `UNIQUE`; amounts are `numeric` with `> 0` checks (migrations `0003` and `0004`); pool transactions must be complete for their status (`0005`).
- **Fact:** queries are static (`concat!`, no `format!`, AGENTS.md §9), which rules out SQL injection through query building.
- **Fact:** applied migrations are immutable (`sqlx` checksums).
- **Fact:** the database is not encrypted by Shield and holds the user ↔ account relation and signed envelopes. Whoever reads it learns what the ASP knows.

### 5.8 Frontend

- **Fact:** spending keys stay in the SDK's browser storage. The auditor token is held in React state only, never in `localStorage` (`apps/web/src/components/audit/audit-view.tsx`).
- **Fact:** no `dangerouslySetInnerHTML` in `apps/web/src`.
- **Fact:** the app sends `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `X-Frame-Options: DENY` and a Content-Security-Policy (`apps/web/src/lib/csp.ts`, set in `apps/web/next.config.ts`). Scripts, workers and fonts come only from the app's origin; data goes only to the app, Shield's API and the RPC; no frames, plugins or foreign form targets. It allows `'wasm-unsafe-eval'` (the SDK compiles WebAssembly) and `'unsafe-inline'` scripts (Next inlines hydration scripts in static pages). `/api-reference.html` has its own policy that adds Redocly's CDN only.
- **Fact:** ramp deposits are signed in the browser with Freighter before they reach the API (`signedIntentRequest` in `apps/web/src/lib/intent-request.ts`).
- **Fact:** the SPP SDK, its WASM, workers and circuits are copied into `apps/web/public/spp/` (generated, gitignored) and loaded at runtime from the same origin (`scripts/stage-web-sdk.sh`). Users trust whoever serves the app (threat model limit 10).
- **Fact:** Next.js 16.0–16.4.0 has critical RCEs; the web pins `next` 16.4.3 (`apps/web/package.json`).

## 6. Existing tests and tooling

| Check | What it covers | Result on this revision |
|---|---|---|
| `make test` (API) | Unit, `#[sqlx::test]` database tests, HTTP tests with port fakes, adapters against `wiremock` | 259 passed, 0 failed, 2 ignored (`testnet_*`) |
| Worker idempotency | Ambiguous submits, dropped and refused transactions, RPC lag, a chaotic mix; each test asserts one deposit and one transfer | Part of the above (`tests/shielding_worker.rs`) |
| Front-running | Dust sent with an intent's memo never funds it | `tests/intent_watcher.rs`; also verified on testnet ([scenarios](docs/scenarios.md#3-money-edge-cases-covered-by-tests)) |
| `make coverage` | `cargo llvm-cov`, `main.rs` excluded | **94.90 %** of lines overall (90.33 % of regions). `src/testing.rs` (fakes) is counted |
| `make deny` | Advisories, licenses, sources (crates.io only), bans | Passes for both crates. `apps/pool-cli/deny.toml` ignores **RUSTSEC-2024-0388** (`derivative`, unmaintained, compile-time dependency of arkworks inside the SPP prover). Duplicate crate versions (`base64`, `sha2`, `digest`, `syn`, …) are warnings by policy (`multiple-versions = "warn"`) |
| Clippy | `-D warnings`, `pedantic` | Passes |
| Mutation check (manual) | Each protection above removed in turn, the tests meant to catch it run | 17 of 18 caught. The survivor is the constant-time token comparison (R8): a timing property no functional test can observe |
| Web tests | Vitest + Testing Library + MSW; Playwright smoke | 97 unit tests (plus 2 type-level tests in `packages/api-types`) and 5 Playwright tests pass, one of them checking the CSP in Chromium |
| gitleaks (CI) | Default rules plus a rule for Stellar secret seeds (`.gitleaks.toml`) | Runs in CI; one reviewed false positive in `.gitleaksignore` |
| CI hygiene | Actions pinned by commit SHA, `permissions: contents: read`, actionlint, shellcheck, OpenAPI lint and codegen drift | `.github/workflows/ci.yml` |
| Dependabot | GitHub Actions, `apps/api` and `apps/pool-cli` (cargo), the pnpm workspace (npm) | `.github/dependabot.yml` |
| pnpm supply chain | pnpm 12's built-in checks (the repo sets no policy of its own); AGENTS.md §9 notes it may block packages published less than a day ago | `pnpm install --frozen-lockfile` reported "Lockfile passes supply-chain policies" |

**Not used today** (the skill lists them): Scout and OpenZeppelin's Security Detectors SDK only analyse Soroban contracts, and Shield has none of its own. Formal verification and contract monitoring belong to the pool's operator; see §8.

## 7. Known limitations, risks and open questions

### 7.1 Known limitations (accepted for the hackathon)

These are limits 1–11 of the [threat model](docs/threat-model.md#known-limits-said-before-anyone-asks). In short:

| # | Limit | Planned fix |
|---|---|---|
| L1 | Amount and timing correlation between the anchor's payment and the deposit; tiny anonymity set | P4 |
| L2 | The anchor knows the user and the amount | Inherent |
| L3 | Direct mode reveals the signer | P3 (relayer) |
| L4 | Shield briefly holds the funds and the intermediate note | Protocol level 2 |
| L5 | The asset issuer could enable freeze or clawback | — |
| L6 | Shield and the auditor are trusted parties | Future work |
| L7 | Unaudited contracts; single-key pool admin | — |
| L8 | ~7-day RPC history window | SPP bootnode |
| L9 | `GET /v1/users/{account}` is public | P2 |
| L10 | Users trust whoever serves the web app; the CSP still allows inline scripts | O2 |
| L11 | The rate limit is per client IP (global behind a proxy) | O3 |

### 7.2 Risks found by the first review, and their status

| # | Risk | Status |
|---|---|---|
| R1 | `POST /v1/intents` needed no authentication: anyone who knew an allowed account could open SEP-24 deposits at the anchor in that user's name | **Fixed.** Signed, fresh, single-use requests (P10). Tests: `only_the_account_can_start_its_deposit`, `a_stale_or_future_request_is_refused`, `a_replayed_request_starts_no_second_deposit`, `an_unsigned_request_is_refused` (`tests/http_intents.rs`), `a_request_id_creates_at_most_one_intent` (`tests/intent_service.rs`), the message vectors in `asp/ownership.rs` and `apps/web/src/lib/intent-request.test.ts` |
| R2 | No rate limiting | **Mitigated.** Per-IP limit on onboarding and intents (§5.6). Tests: `http/rate_limit.rs`, `tests/http_rate_limit.rs`. Limits: see L11 |
| R3 | `blocked` users were not stopped mid-intent | **Fixed** (P11). Tests: `blocking_rejects_the_users_unfunded_intents_and_refuses_new_ones`, `unblocking_does_not_revive_rejected_intents` (`tests/asp_service.rs`), `a_blocked_user_gets_no_deposit_and_the_funds_stay_in_custody`, `a_user_blocked_after_the_deposit_never_receives_the_transfer`, `a_failed_transfer_is_not_rebuilt_for_a_user_blocked_meanwhile` (`tests/shielding_worker.rs`), `a_block_while_the_anchor_answers_refuses_the_intent` (`tests/asp_service.rs`) |
| R4 | No Content-Security-Policy | **Fixed** (§5.8), with `'unsafe-inline'` scripts still allowed. Tests: `apps/web/src/lib/csp.test.ts` and the CSP test in `apps/web/e2e/smoke.spec.ts` (WebAssembly compiles; `eval` and requests to other origins are blocked; no violations on any page) |
| R5 | Dependabot did not watch `apps/pool-cli` or npm | **Fixed** (`.github/dependabot.yml`) |
| R6 | Nothing checked that the operator and intake seeds differ | **Fixed** (§5.6). Test: `operator_and_intake_must_be_different_keys` (`config.rs`) |
| R7 | `SOROBAN_RPC_URL` and `HORIZON_URL` accepted `http://`; Shield trusts a single node | **Partly fixed:** `https` required outside loopback (§5.2; test `rpc_and_horizon_need_https_except_on_this_machine`). Trusting one node is unchanged (§8) |
| R8 | The token comparison revealed the token's length | **Fixed:** digests are compared (§3.1). Test: `a_prefix_or_an_extension_of_the_token_is_refused` (`http/extract.rs`) |

### 7.3 Hypotheses of the first review, now verified

| # | Hypothesis | Result |
|---|---|---|
| H1 | `GET /v1/intents/{id}` exposed a session token in `interactive_url` | **Confirmed** in the sandbox's source (`Lendara-Org/Lendara-monorepo`, `apps/api/internal/handlers/sep24.go`: a one-time token valid for 15 minutes; whoever redeems it first gets the widget session). **Fixed:** only the creation response carries the URL. Test: `creates_and_reads_an_intent` (`tests/http_intents.rs`) |
| H2 | The 15 s router timeout could cut onboarding while it waited 30 s per ASP write | **Confirmed** by reading `http/mod.rs` and `asp/service.rs`. **Fixed:** one 30 s budget per request and a 105 s timeout on ASP routes (§5.2). Tests: `asp_routes_outlast_the_wait_for_the_chain` (`http/mod.rs`), `one_request_waits_for_the_chain_within_one_budget` (`tests/asp_service.rs`) |
| H3 | An intent could stay `awaiting_anchor` forever when the anchor's payment could not be confirmed | **Confirmed. Fixed** (P12). Tests: `an_anchor_payment_that_never_shows_up_ends_in_review`, `a_completed_report_without_its_transaction_ends_in_review`, `a_payment_inside_the_grace_period_still_funds_the_intent` (`tests/intent_watcher.rs`) |
| H4 | A slow `pool-cli` run blocked the watcher and the anchor sync | **Confirmed. Fixed:** separate loops (§5.3). Test: `a_stuck_prover_holds_up_neither_payments_nor_the_anchor` (`tests/intent_watcher.rs`), which also checks that shutdown does not wait for a hung proof |

### 7.4 Still open

| # | Item | Why it is not fixed here |
|---|---|---|
| O1 | `GET /v1/users/{account}` is public (L9) | The web polls it while onboarding is pending; signing every read is not workable. Needs sessions derived from the account's signature (P2) |
| O2 | The CSP allows inline scripts | Next's statically rendered pages inline their hydration scripts without a nonce; nonces need dynamic rendering of every page, a product and hosting decision |
| O3 | Rate limit by client IP only (L11) | Behind a proxy the limit becomes global; trusting `X-Forwarded-For` needs to know the deployment (H10). An attacker with many addresses and fresh accounts can still make the operator pay for many ASP writes: a global cap or a cost to onboard is a product decision |
| O4 | Shield trusts one RPC and one Horizon node for statuses | Cross-checking needs a second provider and a policy for disagreements |
| O5 | A user blocked mid-intent ends in `error` with the funds in Shield's custody | Refunding them (to whom, through the anchor?) is an operational and compliance decision; the funds are safe and flagged for review |
| O6 | A completed intent's `interactive_url` and the anchor's token are still stored in the database | The anchor's token is single-use and expires in 15 minutes; deleting it is housekeeping, not a fix |

## 8. Recommendations

None of these are implemented. They are ordered by how directly they protect funds or privacy.

1. **Enable GitHub private vulnerability reporting** for this repository (§10).
2. **Sessions for user reads** (O1): authenticate `GET /v1/users/{account}` with a session derived from the account's SEP-53 signature, as planned in P2.
3. **A global cap on onboardings** per hour, or another cost to onboard (O3), so fresh accounts from many addresses cannot drain the operator.
4. **Alert on custody**: intents in `error` after `funds_received`, intents ended by the anchor-payment grace (P12), and new unmatched rows in `intake_payments`. The states exist and are logged at `error` level; nothing pages a person yet.
5. **Cross-check statuses** against a second RPC/Horizon provider before replacing a transaction (O4).
6. **CSP nonces** if the pages can be rendered per request (O2).
7. **Monitor the asset issuer's flags** (L5) and the RPC history window (L8).
8. **Before any mainnet discussion:** an audit of SPP and of Shield (the skill points to SDF's Audit Bank), multisig for the pool admin (L7), and key storage in an HSM/KMS.

## 9. Checklist for changes

Use it in every PR that touches the areas below, together with AGENTS.md §5 and §8.

**Keys and secrets**
- [ ] No secret seed (`S…`), token, view key or note secret in a versioned file, fixture, test name or log; `.env.example` holds empty values only.
- [ ] New secrets are `SecretString` and reach `pool-cli` only through `FORWARDED_ENV` (or `AUDITOR_ENV` for the GVK).
- [ ] Operator, intake and auditor keys stay separate; nothing new is signed with the wrong one.

**Money**
- [ ] Every new on-chain write follows prepare → store hash → submit → status, and a retry checks the stored hash before preparing a new transaction.
- [ ] Every intent status change goes through `check` (`intent/state.rs`) and `transition` (`store/intents.rs`).
- [ ] Money moves only to a recipient whose ASP status is `allowed`, checked right before the step that sends it.
- [ ] Every wait has a bound and a terminal state; nothing is rechecked forever.
- [ ] Amounts are stroops or decimal strings; no `as` casts on amounts.
- [ ] A payment funds an intent only when the anchor reports that very transaction.
- [ ] Worker tests assert exactly one deposit and one transfer, including on the failure paths.

**Stellar**
- [ ] The network passphrase is the testnet one and is checked before signing.
- [ ] Transactions are simulated before they are signed.
- [ ] Assets are matched by code **and** issuer; contract ids come from the deployment file, never from user input.
- [ ] Anything Shield signs for a third party (like a SEP-10 challenge) is fully validated first.
- [ ] Changes to the ASP keep the byte-order rules of AGENTS.md §9.

**Privacy**
- [ ] The user's address still appears in no ramp-mode transaction (check the explorer links with real testnet hashes).
- [ ] Logs carry intent or user ids and route templates, never "address + amount", keys or notes.
- [ ] New public endpoints are listed in §3.3 with their authentication, and in `openapi.yaml`.
- [ ] A public endpoint that costs Shield something (an on-chain write, an anchor request) is signed by the account and rate-limited.
- [ ] Nothing that grants access to a third party (an anchor's interactive URL, a token) is returned by an unauthenticated read.

**Web**
- [ ] No spending key, view key or token goes to `localStorage`, a URL or the API.
- [ ] No `dangerouslySetInnerHTML`; API types come from `packages/api-types` only.

**Dependencies and CI**
- [ ] New dependencies were agreed beforehand (AGENTS.md §5.1); `make deny` passes; any new `ignore` entry has a reason.
- [ ] SPP stays pinned to one tag across contracts, crate and npm package.

**This document**
- [ ] If the change adds an asset, an actor, an endpoint, a limit or a risk, update this file and the threat model in the same PR.

## 10. Reporting a vulnerability

**Do not open a public issue, discussion or pull request with vulnerability details.**

Today the repository has **no private reporting channel**: GitHub's private vulnerability reporting is disabled for `Lendara-Org/lendara-shield`, and no security contact is published. Until maintainers enable one:

1. Open a GitHub issue that asks the maintainers for a private contact, **with no technical details**.
2. Once you have a private channel, send the affected component and revision, the impact, reproduction steps on **testnet** (never mainnet), and any transaction hashes.

Shield runs on testnet with no real funds, so there is no bug bounty. Vulnerabilities in the SPP contracts or SDK belong to Nethermind's [stellar-private-payments](https://github.com/NethermindEth/stellar-private-payments) project. Vulnerabilities in Stellar core, SDKs or CLI belong to Stellar's [bug bounty](https://immunefi.com/bug-bounty/stellar/).
