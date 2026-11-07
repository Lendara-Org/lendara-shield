# Threat model

What Lendara Shield hides, from whom, and the limits we know about. Shield runs on **testnet**, over a developer-preview protocol with **unaudited contracts**. Tranche 2 extends this document into a STRIDE-style threat model with a monitoring plan for production.

## Who sees what

| Observer | Ramp mode | Direct mode |
|---|---|---|
| **Public** (explorer, RPC) | The anchor paid Shield's omnibus account; Shield deposited into the pool; Shield made private transfers. **Not the user's address**, not who received what | The user's address deposits (amount visible), signs their own private transfers (amount and recipient hidden) and withdraws (amount visible) |
| **The funding source** | The user and the amount (it takes the payment and applies its own checks). Not what happens inside the pool | — |
| **Shield** (operator, ASP) | Which Stellar account owns which pool keys (it is the ASP); every intent: account, amount, hashes | Which account owns which keys |
| **The auditor** (Global View Key) | Every note in the pool: owner key and amount, which Shield maps to accounts | Same |
| **Another user** | Nothing about others' balances or transfers | Same |

## Assets and how they are protected

| Asset | Where it lives | Protection |
|---|---|---|
| Users' spending keys | Only in the user's browser (derived from a Freighter signature, kept in the SDK's browser storage) | Never sent to Shield; Shield only receives public keys and the ASP leaf |
| Global View Key (auditor) | A file on Shield's server (`SHIELD_GVK_KEY_FILE`) | Only passed to the auditor's `pool-cli` process; never to the shielding worker, the browser or logs; the audit endpoint needs `AUDITOR_TOKEN` (constant-time check) |
| Operator key (pool admin, ASP) | Server environment | Separate from the omnibus and auditor keys (the API refuses to start if the operator and omnibus seeds are equal); admin endpoints need `ADMIN_TOKEN` |
| Omnibus (intake) key and its pool wallet | Server environment and an SQLite wallet outside the repo | Separate key; `pool-cli` runs with an empty environment plus only what it needs |
| Funds in custody | The omnibus account, between the anchor's payment and the private transfer | Idempotent worker (hash stored before sending, never re-prepared while it may have landed); repeated failures stop in `error` for manual review |
| The user ↔ key relation | Shield's database | Never in logs: logs carry intent ids and route templates, never "address + amount" |
| An account's onboarding | `POST /v1/onboarding` | The request carries the account's SEP-53 signature (Freighter `signMessage`) over the exact keys and leaf, so nobody can attach their keys to someone else's account |
| Starting a deposit | `POST /v1/intents` | The request carries the account's SEP-53 signature over the currency, the amount, a single-use `request_id` and `issued_at` (at most 5 minutes old), so nobody else can open deposits at the anchor in a user's name, and a captured request cannot be replayed |
| The anchor's widget session | The SEP-24 interactive URL (it carries the anchor's one-time token) | Returned only to the signed request that created the intent; `GET /v1/intents/{id}` never includes it |
| Shield's operator funds and the anchor | Onboarding (on-chain ASP writes) and intent creation (anchor deposits) | Per-client-IP rate limit (`RATE_LIMIT_PER_MINUTE`, default 20) |
| A blocked user's money | Intents of a user the ASP blocks | Unfunded intents end `rejected` when the block is confirmed; a funded one is stopped before its deposit, its transfer or a rebuilt transfer, and ends `error` with the funds in Shield's custody, never sent to the frozen key |
| Funding an intent | The watcher and the anchor sync | Intent memos are random (63 bits). A payment with an intent's memo only funds it if the anchor reports that very transaction (`stellar_transaction_id`); anything else waits for review, so dust sent with someone's memo cannot hijack their deposit |

## Known limits (said before anyone asks)

1. **Amount and timing correlation at the ramp.** The anchor's payment to Shield and Shield's pool deposit carry the **same amount** within seconds, and the testnet anonymity set is tiny. An observer cannot see the user's address, but can link "this anchor payment" to "this deposit". Mitigation, not built yet: batched deposits, rounded amounts and random delays from the omnibus account (Tranche 3).
2. **The funding source knows the user and the amount.** That is inherent to any party that takes the payment. Shield removes the public link, not the funding source's knowledge.
3. **Direct mode reveals the signer.** Whoever signs a pool transaction is its source account, so in direct mode the user's address shows up as a pool user on every transfer (amounts and recipients stay hidden). Mitigation: a fee-bump relayer (Tranche 1). The web says this next to the button.
4. **Custody.** In ramp mode Shield's omnibus account briefly holds the user's funds, and Shield holds the pool keys of that intermediate note. That is the same trust model as any custodial ramp step; the protocol's level 2 (anchors depositing natively into the pool) removes it.
5. **The asset issuer can still act.** The sandbox USDC issuer has neither `AUTH_REVOCABLE` nor `AUTH_CLAWBACK_ENABLED` today, but it is not `AUTH_IMMUTABLE`: it could enable them later and freeze or claw back the pool's balance. The issuer also appears as a visible account in every transfer of its asset.
6. **Shield and the auditor are trusted parties.** Shield knows which account owns which keys (it is the ASP); the auditor can decrypt everything. That is the compliance trade-off of a permissioned pool. Splitting the roles across independent parties is future work.
7. **Unaudited, developer-preview contracts.** Stellar Private Payments `sdk-v0.2.1`. The pool admin is a single key (no multisig) on testnet.
8. **History window and the bootnode.** Proving needs the pool's full event history; the public RPC keeps about 7 days. Shield runs SPP's bootnode for the older events (the first pool outlived the window before it existed and was redeployed on 6 November). Clients trust the bootnode for that history: it could omit or forge old events or point the handoff at the wrong ledger, so a wrong answer makes proofs fail rather than steal funds (the contracts verify every proof). It also sees the IP and timing of every client that syncs through it. Anyone can run their own bootnode or an archival RPC instead.
9. **`GET /v1/users/{account}` is public.** It tells anyone whether an account onboarded with Shield, its eligibility and allow-list status, and the hash of its allow-list insertion: the same "this address uses the pool" signal we avoided by not using SPP's public key registry. Fix (not built yet): require a session derived from the account's signature to read its status. A signature on every read is not an option: the web polls this endpoint while onboarding is pending, so it needs sessions first (Tranche 1). `GET /v1/intents/{id}` is also unauthenticated, but needs the intent's random id and returns no account and no interactive URL.
10. **Browser trust.** The circuit lock is pinned when the web app is built, so circuit artifacts served next to it are checked against it, but users still trust the app and whoever serves it. A Content-Security-Policy limits scripts, workers and network access to the app's origin, Shield's API, the RPC and the bootnode; it still allows inline scripts, because Next inlines them in statically rendered pages.
11. **The rate limit is per client IP.** Behind a reverse proxy every client shares the proxy's address, so the limit becomes global. An attacker with many addresses can still onboard many fresh accounts (each costs the operator an on-chain write).
12. **The RPC provider.** Shield, `pool-cli` and the browser read the chain and submit through one Stellar RPC (`SOROBAN_RPC_URL`, `NEXT_PUBLIC_SOROBAN_RPC_URL`). Whoever runs it (SDF's public endpoint, a provider such as Gateway.fm, or Shield itself) sees the IP and timing of every client and which contracts it reads, and could delay, omit or misreport answers. It cannot forge a spend: the contracts verify every proof and signature. But Shield trusts its answers for bookkeeping: `pool-cli` reads transaction status from it, and the worker uses that status to complete an intent or to replace a transaction it believes was lost. A malicious RPC could make an intent look completed, or trigger a replacement of a deposit that already landed (the worker's sequence and grace checks limit the second case), and the browser trusts it for balances and sync. Its latency also sets how fast shielding is. A self-hosted RPC removes the third party.

## Not built yet

Sessions and project API keys and the relayer (Tranche 1); key management in a KMS/HSM and per-note selective disclosure for users (Tranche 2); production hosting and anonymity-set hardening (Tranche 3); proxy-aware or global rate limiting.
