# Threat model (short)

What Lendara Shield hides, from whom, and the limits we know about. This is a hackathon build on **testnet**, over a developer-preview protocol with **unaudited contracts**.

## Who sees what

| Observer | Ramp mode | Direct mode |
|---|---|---|
| **Public** (explorer, RPC) | The anchor paid Shield's omnibus account; Shield deposited into the pool; Shield made private transfers. **Not the user's address**, not who received what | The user's address deposits (amount visible), signs their own private transfers (amount and recipient hidden) and withdraws (amount visible) |
| **The anchor** | The user and the amount (it does the KYC and takes the payment). Not what happens inside the pool | — |
| **Shield** (operator, ASP) | Which Stellar account owns which pool keys (it is the ASP); every intent: account, amount, hashes | Which account owns which keys |
| **The auditor** (Global View Key) | Every note in the pool: owner key and amount, which Shield maps to accounts | Same |
| **Another user** | Nothing about others' balances or transfers | Same |

## Assets and how they are protected

| Asset | Where it lives | Protection |
|---|---|---|
| Users' spending keys | Only in the user's browser (derived from a Freighter signature, kept in the SDK's browser storage) | Never sent to Shield; Shield only receives public keys and the ASP leaf |
| Global View Key (auditor) | A file on Shield's server (`SHIELD_GVK_KEY_FILE`) | Only passed to the auditor's `pool-cli` process; never to the shielding worker, the browser or logs; the audit endpoint needs `AUDITOR_TOKEN` (constant-time check) |
| Operator key (pool admin, ASP) | Server environment | Separate from the omnibus and auditor keys; admin endpoints need `ADMIN_TOKEN` |
| Omnibus (intake) key and its pool wallet | Server environment and an SQLite wallet outside the repo | Separate key; `pool-cli` runs with an empty environment plus only what it needs |
| Funds in custody | The omnibus account, between the anchor's payment and the private transfer | Idempotent worker (hash stored before sending, never re-prepared while it may have landed); repeated failures stop in `error` for manual review |
| The user ↔ key relation | Shield's database | Never in logs: logs carry intent ids and route templates, never "address + amount" |
| An account's onboarding | `POST /v1/onboarding` | The request carries the account's SEP-53 signature (Freighter `signMessage`) over the exact keys and leaf, so nobody can attach their keys to someone else's account |
| Funding an intent | The watcher and the anchor sync | Intent memos are random (63 bits). A payment with an intent's memo only funds it if the anchor reports that very transaction (`stellar_transaction_id`); anything else waits for review, so dust sent with someone's memo cannot hijack their deposit |

## Known limits (said before anyone asks)

1. **Amount and timing correlation at the ramp.** The anchor's payment to Shield and Shield's pool deposit carry the **same amount** within seconds, and the testnet anonymity set is tiny. An observer cannot see the user's address, but can link "this anchor payment" to "this deposit". Mitigation, not built yet: batched deposits, rounded amounts and random delays from the omnibus account (plan item P4).
2. **The anchor knows the user and the amount.** That is inherent to fiat ramps with KYC. Shield removes the public link, not the anchor's knowledge.
3. **Direct mode reveals the signer.** Whoever signs a pool transaction is its source account, so in direct mode the user's address shows up as a pool user on every transfer (amounts and recipients stay hidden). Mitigation: a fee-bump relayer (P3). The web says this next to the button.
4. **Custody.** In ramp mode Shield's omnibus account briefly holds the user's funds, and Shield holds the pool keys of that intermediate note. That is the same trust model as any custodial ramp step; the protocol's level 2 (anchors depositing natively into the pool) removes it.
5. **The asset issuer can still act.** The sandbox USDC issuer has neither `AUTH_REVOCABLE` nor `AUTH_CLAWBACK_ENABLED` today, but it is not `AUTH_IMMUTABLE`: it could enable them later and freeze or claw back the pool's balance. The issuer also appears as a visible account in every transfer of its asset.
6. **Shield and the auditor are trusted parties.** Shield knows which account owns which keys (it is the ASP); the auditor can decrypt everything. That is the compliance trade-off of a permissioned pool. Splitting the roles across independent parties is future work.
7. **Unaudited, developer-preview contracts.** Stellar Private Payments `sdk-v0.2.1`. The pool admin is a single key (no multisig) on testnet.
8. **History window.** Proving needs the pool's full event history; the public RPC keeps about 7 days. Without SPP's bootnode, a pool deployed on 26 October can be used until about 2 November.
9. **`GET /v1/users/{account}` is public.** It tells anyone whether an account onboarded with Shield, its KYC and allow-list status, and the hash of its allow-list insertion: the same "this address uses the pool" signal we avoided by not using SPP's public key registry. Fix (not built yet): require the account's signature (or a session from it) to read its status.
10. **Browser trust.** The circuit lock is pinned when the web app is built, so circuit artifacts served next to it are checked against it, but users still trust the app and whoever serves it.

## Out of scope for the hackathon

Production hosting, key management (HSM/KMS), rate limiting and project API keys (P2), the relayer (P3), anonymity-set hardening (P4), per-note selective disclosure for users (P5).
