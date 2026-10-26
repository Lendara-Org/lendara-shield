# AGENTS.md — Lendara Shield

> Project context and rules for everyone working on this repo and their AI assistants (Claude Code, Cursor, Codex, Copilot…). Read it before any task.
>
> - **Language:** code, names, comments, commits, PR titles and descriptions, and this file: **English**.
> - **Personal notes:** anyone can keep a `CLAUDE.local.md` at the root (it is in `.gitignore`; Claude Code loads it automatically). What applies to the whole team goes here.
> - If you change a decision or a convention, or find a new pitfall, **update this file in the same PR**.

---

## 1. What Lendara Shield is

Lendara Shield is the reference implementation of the **Shield Protocol**: a compliant privacy layer for Stellar that connects fiat on-ramps (SEP-24 anchors) to a [Stellar Private Payments](https://developers.stellar.org/docs/build/apps/privacy) (SPP) pool.

**The problem.** Stellar has privacy tools in developer preview, but privacy breaks at the fiat edge: a ramp deposit ties a user's address to an amount and a time. A privacy pool with an allow-list also needs someone to run KYC, maintain the list (the ASP), hold the audit key and pay fees.

**The idea.** An anchor already runs KYC and an omnibus account. If the anchor (or Shield on its behalf) deposits into the pool from its omnibus account and then transfers privately to the user's keys, **the user's address never appears on-chain**.

**Tagline:** *KYC once, pay privately, disclose selectively.*

**Team:** Anouk (Costa Rica), Karu and Elisa (Argentina). Built for HackMeridian 2026 (Genesis track).

**Related repo:** `Lendara-Org/Lendara-monorepo` is Lendara's SEP-1/10/24 sandbox anchor. Shield consumes it like any SEP-24 wallet would and **never modifies it from here**.

### Glossary

| Term | Meaning here |
|---|---|
| Pool / SPP | Stellar Private Payments privacy pool (Nethermind): a Soroban contract with notes, commitments, nullifiers and Groth16 proofs |
| Note | A private value in the pool that only its owner can spend |
| `ext_amount` | Public amount of a pool operation: positive on deposit, **0 on a private transfer**, negative on withdraw |
| ASP | Association Set Provider: keeps the pool's allow-list and block-list. **Shield is the ASP** |
| Intake account | Shield's omnibus account: receives the anchor's USDC, deposits into the pool |
| Global View Key (GVK) | The pool's audit key. Shield keeps it; it never reaches a browser |
| Intent | One ramp operation (local currency → private balance) |

## 2. Two modes, one core

| | **`ramp`** | **`direct`** |
|---|---|---|
| For | Users paying in local currency | Users who already hold USDC |
| Money in | Fiat → SEP-24 anchor → intake → pool → private transfer to the user | The user deposits from their own wallet |
| User address on-chain | **Never** | Yes when depositing, and as the signer of their own transfers (until a relayer exists) |
| Depends on an anchor | Yes (any SEP-24 anchor) | No |

- **Ramp mode is optional, enabled by configuration.** Without `RAMP_ANCHOR_HOME_DOMAIN`, intent endpoints answer `409 ramp_mode_disabled` and the web hides the option. Direct mode always works.
- **Ramp mode talks to the anchor only through SEP-1, SEP-10 and SEP-24.** Shield is a SEP-24 *client*: it authenticates with its intake account (SEP-10 with an `id` memo per user) and requests an interactive deposit with `account = intake` and an `id` memo per intent. This is central to the protocol pitch: no anchor changes.
- **Both modes share** user records, the ASP, the pool client, the auditor and the state machine. Don't duplicate logic.

### Ramp flow end to end

1. The user connects Freighter in the web; the SPP browser SDK derives their pool keys. The web sends Shield the public keys, the ASP leaf (`deriveAspUserLeaf`) and a SEP-53 signature by the account over them. The user does **not** register in SPP's public key registry (that would publish their address).
2. Simulated KYC → if approved, Shield (operator key) inserts the leaf into the allow-list.
3. `POST /v1/intents`, signed by the account (SEP-53 over the amount, a single-use `request_id` and `issued_at`) → SEP-10 against the anchor (intake + user memo) → SEP-24 interactive deposit to the intake with the intent's memo → the interactive URL (only in this response: it carries the anchor's one-time token).
4. The user completes the anchor's widget; the anchor pays USDC to the intake with the memo.
5. The watcher records the payment (Horizon). The intent is funded **only** with the payment the anchor reports as its own (`stellar_transaction_id`), checking asset code and issuer.
6. The worker deposits into the pool from the intake (`ext_amount > 0`), then transfers privately to the user's keys (`ext_amount = 0`).
7. The intent is `completed`; the user sees the private balance.

**Honest limit (in the threat model and the pitch):** the anchor knows the user and the amount; to an outside observer, the anchor's payment and the pool deposit have the same amount within seconds, and the testnet anonymity set is tiny. Mitigation (batching, rounded amounts, random delay) is P4.

## 3. The Shield Protocol

Level 1 (Shield as middleware for any SEP-24 anchor) is implemented. Level 2 (anchors depositing natively into the pool: `SHIELD_*` in `stellar.toml`, `features.shielded_deposit`, `destination_type: shielded`) is a draft in [`docs/protocol/shield-protocol.md`](docs/protocol/shield-protocol.md). The standard's name has no "Lendara" in it so other anchors can adopt it; the product and repo keep the brand.

## 4. Architecture

```
lendara-shield/
├── apps/api/            Rust (Axum): ASP, intents, SEP-1/10/24 client, Horizon watcher,
│                        shielding worker, auditor. Ports and adapters; Postgres (sqlx)
├── apps/pool-cli/       Rust: stateless JSON CLI around the SPP SDK (prove, sign, submit, GVK audit)
├── apps/web/            Next.js: onboarding, deposit (both modes), private wallet, audit
├── packages/api-types/  TypeScript types generated from openapi.yaml
├── openapi.yaml         The API contract: SOURCE OF TRUTH
├── docs/                Protocol draft, threat model, scenarios, SPP spike, demo script
├── scripts/             setup-testnet, init-intake, verify-testnet, stage-web-sdk
└── Makefile             Every command (make help)
```

- `apps/api` is not part of the pnpm workspace (which holds `apps/web` and `packages/*`); the root `Makefile` drives everything.
- **`openapi.yaml` is the contract** and changes before the code. TS types are generated from it (`make codegen`); Rust types are written by hand.
- **The prover lives in `apps/pool-cli`**, run by the API as a subprocess behind the `PoolClient`/`PoolAuditor` ports: the SPP SDK links a SQLite (`rusqlite`) that conflicts with `sqlx`. The state machine and idempotency stay in the API.
- SPP contracts are not copied here: we deploy Nethermind's, pinned to tag `sdk-v0.2.1` (see `docs/spikes/spp.md`).

## 5. Rules

### 5.1 Git and PRs

- **Conventional Commits** with a scope: `api`, `web`, `types`, `protocol`, `repo`, `docs`. Example: `feat(api): add ramp intents state machine`. Enforced by commitlint.
- **Branches:** `<type>/<scope>-<short-description>`, e.g. `feat/api-asp-onboarding`.
- **Flow:** features → `develop` (squash); `develop` → `main` only for releases.
- **Small PRs, one topic each**, ideally under 400 changed lines (excluding generated code, lockfiles and fixtures). Split commits by topic within a PR. Use the PR template (What, Why, How to test, Out of scope, testnet hashes).
- **AI assistants prepare changes; a person reviews, commits, pushes and opens the PR.**
- Ask before adding a dependency; mention its version and why.

### 5.2 Security

- **Never** write Stellar secret seeds (`S…`), API keys, passwords, view keys or note spending keys in versioned files. Secrets go in environment variables; keep `.env.example` current with empty values.
- **Testnet only.** Never use mainnet or its passphrase.
- **Separation of duties:** the operator (pool admin and ASP), the intake (omnibus) and the auditor (GVK) are **three different keys**, never reused.
- **Shield never holds users' spending keys.** Pool keys are derived in the user's browser; Shield only knows public keys.
- **Log hygiene:** logs never include view keys, notes, note secrets, or the pair "user address + amount". Use the intent id; HTTP spans use route templates, never raw paths. The database may hold that relation (Shield is the ASP); logs may not.
- The GVK file is passed only to the auditor's `pool-cli` process. The admin and audit endpoints use bearer tokens compared in constant time.

### 5.3 Money integrity

- **Workers are idempotent.** Before sending a transaction, persist its hash and status; on retry, check the network before resending. A pool deposit can never happen twice for the same intent.
- Use the SDK's `prepare_* → prove → sign` (hash known) → **store** → `submit` → `status`, never one-shot `deposit()`/`transfer()` in the worker.
- Amounts are **stroops (`i64`/`i128`) or decimal strings**, never floats.
- Every intent state change goes through a single module and is recorded (§6.4).
- **Only the anchor funds an intent:** a payment carrying an intent's memo is recorded as unconfirmed until the anchor reports that very transaction.

## 6. Functional design

### 6.1 Testnet accounts and asset

| Account | Purpose | Variable |
|---|---|---|
| Operator | Deploys and administers the pool; administers the ASP | `SHIELD_OPERATOR_SECRET` |
| Intake | Omnibus account: receives USDC from the anchor, deposits and transfers privately | `SHIELD_INTAKE_SECRET` |
| Auditor | Global View Key: a **Baby JubJub** keypair (not a Stellar key) from SPP's `tools/gvkey-gen`; its public half is fixed at deploy time | `SHIELD_GVK_KEY_FILE` (path to `{privateKey, publicKey}` JSON, outside the repo) |

- **Asset:** the Lendara sandbox's test USDC (`RAMP_ASSET_ISSUER`), so ramp mode works end to end. The pool needs a SEP-41 token, so `make setup-testnet` deploys (or reuses) its Stellar Asset Contract.
- **The intake's pool keys** are derived server-side from its own seed and stored in its SDK wallet (`SHIELD_INTAKE_WALLET_PATH`, outside the repo). The intake's ASP leaf must be in the allow-list like any user's: `make init-intake` (`scripts/init-intake.sh` → `shield-api init-intake`, idempotent) derives the keys if missing and has the operator insert the leaf only if absent. `setup-testnet.sh` and the server's deploy run it; at startup the API checks both and logs an error if the intake is not ready.
- `make setup-testnet` (`scripts/setup-testnet.sh`, idempotent) creates and funds the operator and intake, adds the trustline, deploys the SAC, generates the GVK, deploys the pool and initialises the intake. It writes `~/.local/share/lendara-shield/testnet/testnet.env` (mode 600); copy its variables into `.env`. `make test-testnet` runs read-only checks against testnet.

### 6.2 Pool configuration

Own SPP pool for the sandbox USDC: **`allowlist-blocklist` + `gvk-traceable`**, contracts from tag `sdk-v0.2.1` deployed with SPP's `deploy.sh`. The allow-list is **append-only** (`insert_leaf`); blocking means inserting the note key into the block-list (`asp-non-membership`, which supports `delete_leaf`).

### 6.3 Data model

- `shield_users`: account, `sep10_memo` (SEP-10 customer memo), note and encryption public keys, ASP leaf, KYC scenario/status, ASP status.
- `asp_events`: on-chain ASP writes (allow, block, unblock) with their hashes.
- `intents` + `intent_events`: mode, status, fiat amount, asset amount (`numeric` read as text), anchor transaction id, **random 63-bit `memo`** (SEP-24 `id` memo), the signed request's `request_id` (`UNIQUE`: replay guard), when the anchor first reported `completed` without a confirmed payment (`anchor_completed_at`), intake payment hash, the signed envelope, hash and sequence of each pool transaction, retry scheduling.
- `intake_payments`: every incoming payment to the intake, matched or not (unmatched = money in custody for review); `watcher_cursors`.

Statuses and enums are `text` + `CHECK`; constraints (`UNIQUE`, `CHECK`, `FOREIGN KEY`) back the Rust validation.

### 6.4 Intent state machine (ramp)

```
created ──(SEP-24 interactive deposit created)──► awaiting_anchor
awaiting_anchor ──(the anchor's payment confirmed)──► funds_received
funds_received ──(pool deposit stored and sent)──► shielding
shielding ──(deposit confirmed; transfer stored and sent)──► transferring
transferring ──(private transfer confirmed)──► completed

Error exits:
  created ──► error                       (the anchor could not start the deposit: POST /v1/intents → 502)
  created | awaiting_anchor ──► rejected  (the ASP confirmed a block of the user)
  awaiting_anchor ──► expired             (the anchor reports expired)
  awaiting_anchor ──► error               (the anchor failed, e.g. KYC rejected in the sandbox; or it reported
                                           `completed` and its payment was not confirmed within 15 minutes)
  funds_received | shielding | transferring ──► error  (with a message; needs manual review; also when the
                                                       user is blocked before the deposit or the transfer)
```

Once funds are in Shield's custody (`funds_received` onwards) an intent only ends `completed` or `error`. Implemented in **one module** (`apps/api/src/intent/state.rs`); an invalid transition is an error, every transition writes an `intent_events` row in the same database transaction (`UPDATE … WHERE status = from`).

### 6.5 Simulated KYC and ASP

- Onboarding: public keys + ASP leaf + **SEP-53 signature by the account** over them (exact text in `apps/api/src/asp/ownership.rs`, mirrored in `apps/web/src/lib/onboarding.ts`).
- Shield's KYC scenarios (sandbox): `approved` → allow-listed; `rejected` → never inserted; `blocked` → inserted then block-listed.
- Ramp mode: the anchor's outcome is chosen in the sandbox widget with **test cards** (see `docs/scenarios.md`).
- SEP-24's `amount` is in the asset; Shield sends `source_asset = iso4217:<currency>` and the widget quotes. `amount_fiat` is the user's intent; `amount_asset` is the USDC actually received.
- Rules live in one module (`apps/api/src/asp/`).

### 6.6 Audit

- `GET /v1/audit/transactions` (`AUDITOR_TOKEN`): pool activity decrypted with the GVK (via `pool-cli audit`), notes attributed to Shield users, kind and `ext_amount` from the note balance.
- `GET /v1/audit/public/{tx_hash}`: what anyone sees (Horizon), for the side-by-side comparison in `/audit`.

### 6.7 API

`openapi.yaml` is authoritative. Summary: `GET /health`, `GET /v1/info`, `POST /v1/onboarding`, `GET /v1/users/{account}`, `POST /v1/intents`, `GET /v1/intents/{id}`, `POST /v1/asp/block` and `/unblock` (`ADMIN_TOKEN`), `GET /v1/audit/transactions` (`AUDITOR_TOKEN`), `GET /v1/audit/public/{tx_hash}`. Errors are `{ "error": "<snake_case_code>", "message": "…" }` with stable codes.

### 6.8 Web

Every page lives under `/<lang>/` (`en`, `es`): `/` redirects by the browser's language and old unprefixed paths redirect to `/en/…`. App copy is in `apps/web/src/content/messages/{en,es}.ts` (Spanish must have the same keys; a test checks it); docs pages in `apps/web/src/content/docs/{en,es}.tsx`. Add text in both languages in the same PR.

Routes: `/` (what Shield is), `/docs` (overview, protocol, quickstart, API, scenarios, security), `/onboard`, `/deposit` (ramp with a live intent timeline, or direct with a visible-address warning), `/wallet` (private balance, private address `shield:<note key>:<encryption key>`, transfer, withdraw), `/audit`. Freighter and the SPP SDK sit behind the `ShieldWallet` interface so pages are tested with a fake.

## 7. Conventions and quality

### Rust

- `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings`; `pedantic` warns; `unwrap`, `expect`, `panic`, `dbg`, `todo` and `print_stdout` are denied outside tests; `#![forbid(unsafe_code)]`.
- No `as` for amounts or signed integers: `TryFrom` and an error.
- **Ports and adapters:** the domain never imports `sqlx`, `reqwest`, `axum` or an SDK; services get ports injected (`Arc<dyn Trait>`); handlers stay thin.
- **Newtypes validated in the constructor** (`AccountId`, `TxHash`, `PoolField`, `DecimalAmount`…): if it exists, it is valid.
- **Injected clock** (`trait Clock`): no `Utc::now()` in the domain.
- One `thiserror` enum per module; one `ApiError` mapped to HTTP. Secrets in `secrecy::SecretString`.
- Every outbound HTTP client has a timeout; retries with backoff and a cap only for idempotent operations.
- Workers take work with `next_attempt_at` and status guards; graceful shutdown with `CancellationToken`.
- `///` on everything public in the domain and ports; comments explain *why*.

### Visual identity

- The existing web screens and flows remain the product baseline. Apply Paper identity through `apps/web/src/app/design-tokens.css` and shared primitives; do not recreate Paper screens.
- Token values come from `docs/design-system/tokens.css`; the runtime copy uses Tailwind `@theme static` to expose utilities. Keep their values aligned. Use Sora 600 for headings, Inter for UI and amounts, and Geist Mono for code and identifiers.

- Landing visuals use bilingual HTML/SVG and the pinned @paper-design/shaders-react 0.0.81 MeshGradient. The hero preview switches public/authorized views using fixture data only; never connect it to wallets or backend audit permissions. Stop its automatic cycle on interaction, honor reduced motion (shader speed 0), pause the shader while off screen, render it at the device pixel ratio (not the library's 2x minimum) capped at 1280×720 pixels, and preserve entry links. Entry previews lift on hover without rotation.

- App and docs pages share the home max-width of 1440px, desktop titles at 64/72 and mobile titles at 40/46. Keep text-entry forms bounded (max-width 768px) inside the wider page; data comparisons and docs may use the available width.

### TypeScript

- `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`; no `any`, no `as` unless justified.
- API types only from `packages/api-types`, never written by hand.
- Server components by default; `"use client"` only for wallet, pool SDK or interactive state. Logic lives in tested modules and hooks, not in components.
- Accessibility: labelled form fields, visible loading and error states, keyboard navigation.

### Testing

No code PR without tests for what it adds, **including error paths**.

| Level | What | How |
|---|---|---|
| Unit | Pure domain: transitions, ASP rules, newtypes, amounts, memo matching, SEP-10 validation, SEP-53 proof | `#[test]` in the same file |
| Database | Stores, constraints, transactions | `#[sqlx::test]` (isolated database per test) |
| HTTP | Every endpoint: success, 400, 401, 404, 409, 503 | `build_app()` + `oneshot` with port fakes |
| Adapters | Horizon, RPC, anchor, pool-cli | `wiremock` with recorded public testnet responses; a fake CLI script |
| Worker | Idempotency under failures (ambiguous submit, dropped, refused, RPC lag, chaos) | `FakePool` enforcing the sequence rule; every test asserts exactly one deposit and one transfer |
| Web | Logic, components, API mocks; a Playwright smoke | Vitest + Testing Library + MSW; Playwright |
| Testnet | Real checks | `#[ignore]` tests and `make test-testnet` |

No `sleep` in Rust tests (fake clock). Test names describe behaviour. Coverage with `make coverage`: ≥ 90% in `domain/`, `intent/`, `asp/`; ≥ 75% overall; never chased with empty tests.

## 8. Commands and verification

```bash
make help            # every target
make db-up           # local Postgres (docker compose)
make dev             # API :8080 + web :3000
make lint typecheck test build
make test-web-e2e    # Playwright smoke (after make build-js)
make coverage deny   # Rust coverage, licenses and advisories
make codegen         # TS types from openapi.yaml
```

Before handing over a PR: format, lint, typecheck, tests and build of what changed; `make codegen` if `openapi.yaml` changed; if it touches the network, run the testnet check and include real hashes with stellar.expert links. CI runs all of this plus gitleaks, actionlint and shellcheck.

## 9. Known pitfalls

- **SPP SDK and `sqlx` cannot share a binary** (`libsqlite3-sys` conflict): that is why `pool-cli` exists.
- **Byte order (verified on testnet):** the SDK prints the ASP leaf big-endian and the note public key **little-endian**. The block-list key is the note key with its bytes reversed (`PoolField::field_le`); the GVK audit also reports owners in that reversed form. The encryption key is X25519, not a field element. Validate field elements against BN254 in the right order.
- Block-list convention: `key = value = note public key` (as SPP's app does). Every block-list write invalidates proofs built before it: the worker re-proves and retries; the SDK may ask for an ASP membership resync (`not_allowed`, transient).
- **SDK pool transactions have no time bounds** (`valid_until: null`): a `not_found` transaction may only be replaced once the intake's sequence passed its `sequence` **and** it stayed unseen for `UNSEEN_GRACE` (60 s), because the RPC can lag behind a landed transaction.
- The browser SDK only sees the notes of keys derived **in that browser's storage**: derive (idempotent) before any pool operation.
- The SPP browser SDK is not bundled by Next: `scripts/stage-web-sdk.sh` (`make stage-web-sdk`) copies its JS, WASM, workers, this pool's circuits and `deployments.json` into `apps/web/public/spp/` (gitignored). The circuit lock is pinned at build time.
- `apps/web/public/api-reference.html` is generated from `openapi.yaml` by Redocly on every `dev` and `build` (gitignored).
- The audit's `GvkAudit` gives ledgers, not transaction hashes: the API resolves them from Horizon's ledger operations (pool calls only) and caches them. The worker and the auditor share a lock because both open the intake's SQLite wallet.
- Sandbox: `stellar.toml` lives at `lendara-monorepo-fixh.vercel.app` but `WEB_AUTH_ENDPOINT` and `TRANSFER_SERVER_SEP0024` are on `lendaraweb-production.up.railway.app`; the challenge's `web_auth_domain` is the endpoint host. Scenarios are chosen by test card; there are no webhooks (poll `GET /sep24/transaction`). To pay without a browser: `POST /widget-api/sessions` → `/transaction/confirm` → `/transaction/simulate-payment`.
- **Applied migrations are immutable:** `sqlx` stores their checksums and refuses to start if one changes, even a comment. Older migrations still cite sections of a former `CLAUDE.md`; the rules they mean are now in §5.2, §5.3 and §6.4 here.
- `sqlx` 0.9 rejects dynamically built SQL: build queries with `concat!` and macros, never `format!`. Queries are runtime-checked and covered by `#[sqlx::test]`.
- Don't `source .env` in bash: `STELLAR_NETWORK_PASSPHRASE` has spaces and `;`. The `Makefile` loads it correctly.
- SPP: pin `sdk-v0.2.1` (contracts, crate and npm match); npm's `alpha` tag is stale. Build `pool-cli` in release (the debug prover is slow); the first run downloads circuit artifacts.
- **A new pool needs the intake initialised** (`make init-intake`): a fresh allow-list does not contain the intake's leaf and a fresh wallet has no keys. Otherwise every shielding attempt fails (`sdk: no privacy keys found` or `not_allowed: … RegisterAtASP`) and funded intents end in `error` with the USDC in the intake. The allow-list contract only stores its root, so membership is read from its `LeafAdded` events since the pool's `deploymentLedger`.
- RPC history window: proving needs the pool's full event history and the public RPC keeps ~7 days. Past that, run SPP's `tools/bootnode` or redeploy.
- Next.js 16.0–16.4.0 has critical RCEs: the web pins a patched version.
- MSW 3 renamed `onUnhandledRequest` to `onUnhandledFrame`. In Playwright, look for alerts inside `main` (Next's route announcer is also `role="alert"`).
- TypeScript is pinned to 5.x: `openapi-typescript` needs `^5`.
- `stellar-strkey` hides private keys by design; to encode one (tests only) use `PrivateKey(..).as_unredacted()`.
- pnpm may block packages published less than a day ago (`minimumReleaseAge`): use the previous version, don't add exceptions.

## 10. Status and roadmap

**Hackathon (done, Oct 25–26 2026):** scaffold, SPP spike, API skeleton, OpenAPI contract, testnet pool, onboarding and ASP, intents and anchor client, pool CLI, watcher and shielding worker, web, audit panel, docs, and security fixes (anchor-confirmed funding, signed onboarding). Ramp mode completes on testnet with the user's address appearing nowhere on-chain; hashes are in the README.

**Security fixes after the hackathon:** signed, single-use intent requests; the interactive URL only in the creation response; blocked users stopped mid-intent; a bounded wait for unconfirmed anchor payments; separate ramp and shielding loops; per-IP rate limit; ASP routes with their own timeout; a CSP; startup checks on keys and URLs. See [SECURITY.md](SECURITY.md).

**Known gaps:** `GET /v1/users/{account}` is public (threat model, limit 9; needs sessions, P2); deposit amount/timing correlation (P4).

**After the hackathon (suggested order):**

| # | Item |
|---|---|
| P1 | Shield Protocol level 2 spec: full SEP-24 extension, request/response examples, security discussion |
| P2 | Integrators: projects and hashed API keys, auth on every endpoint (including user status) |
| P3 | Relayer: fee-bump for users' pool operations (no XLM needed, signer not revealed) |
| P4 | Anonymity-set hardening: batched, rounded and delayed deposits from the intake |
| P5 | Per-note selective disclosure for users |
| P6 | Agents: agent keys under their owner's KYC, spending limits, owner audit |
| P7 | Second lane: Confidential Tokens (hidden amounts, visible parties) for payroll |
| P8 | `@lendara/shield` TypeScript SDK and a `privacy` option in the widget |
| P9 | Withdraw to fiat (SEP-24 withdraw from the pool) |
| P10 | Native anchor (level 2) inside the sandbox |
| P11 | Quality: contract tests against `openapi.yaml`, full E2E suite, full threat model |
| H10 | Deploy (API + Postgres, web) with HTTPS |
