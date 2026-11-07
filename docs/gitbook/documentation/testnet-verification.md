# Testnet verification (2026-10-28)

Read-only checks of the live deployment (`lendara-shield.duckdns.org`), the Lendara sandbox anchor and the pool contracts on Stellar testnet. No transactions were sent.

Reference ledger: `5437355` (2026-10-28T18:00:15Z).

## 1. End-to-end circuit

### Live deployment

| Check | Result |
|---|---|
| `GET /health` | `200` `{"status":"ok","database":"ok"}` |
| `GET /v1/info` | `200`, ramp and direct enabled, anchor `lendara-monorepo-fixh.vercel.app` |
| `/en`, `/es`, `/en/docs`, `/en/onboard`, `/en/deposit`, `/en/wallet`, `/en/audit` | `200` |
| `/api-reference.html`, `/spp/deployments.json` | `200` |
| `POST /v1/intents` with `{}` | `400 invalid_request` |
| `POST /v1/onboarding` with `{}` | `400 invalid_request` |
| `GET /v1/audit/transactions` without token | `401 unauthorized` |

### Anchor (Lendara sandbox)

| Check | Result |
|---|---|
| `stellar.toml` | testnet passphrase, `WEB_AUTH_ENDPOINT` and `TRANSFER_SERVER_SEP0024` on `lendaraweb-production.up.railway.app`, USDC issuer `GBVIQ7QV…H47B5` |
| SEP-24 `/info` | USDC deposit enabled, min 1, max 10000 |
| SEP-10 challenge | `200` |

### On-chain

- The 8 transactions listed in the README's *Proof on testnet* (also listed in [Technical Architecture](technical-architecture.md), Section 9) are successful on Horizon.
- The intake (`GC5VXVUOU6OHZVZ3KZNFK5CJNOEDQUMOOZI32VHYHYV64GBHVCPTEHPQ`) completed 5 ramp intents on 2026-10-26, each as anchor payment → pool deposit → private transfer, about 20 s apart. No failed transactions.
- The intake's USDC balance is `0`: no funds left in custody.
- Last intake activity: 2026-10-26T15:39:22Z.

### Local tests

| Suite | Result |
|---|---|
| `make test-js` | 110/110 passed (18 files) |
| `make test-pool-cli` | 7/7 passed |
| `make test-api` | Not run: no Docker/Postgres on the machine |

## 2. Configuration and asset status

**Pass criterion:** the web, the API, the anchor and the contracts use the same expected configuration. **Result: passed.**

| Item | Web (`/spp/deployments.json` + bundle) | API (`/v1/info`) | Anchor | On-chain | OK |
|---|---|---|---|---|---|
| Network | testnet, `Test SDF Network ; September 2015`, RPC `soroban-testnet.stellar.org` | same | same | — | ✅ |
| USDC issuer | `GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5` | same | same, also in `ACCOUNTS` | — | ✅ |
| Token (SAC) | `CAE6WLIMEMDSRSZZW7H3O7DJS2DRDV5NTR2FHE53TKGJMJOHWGKSR4QX` | same | — | `stellar contract id asset` gives the same id; `name()` = `USDC:GBVI…` | ✅ |
| Pool | `CDD66GL3HT5JLRIB5BHAV36XFSEAGRXMGRJJ4VK747ODBXEJ77YAC7OF` | same | — | `Token` and `Verifier` match; `DepositsPaused = false` | ✅ |
| ASP allow-list | `CCETAQARCUMNMCLJKQM6UNY2QJDOJZYGI54MXA3FMPJY4VJDMN2L26GB` | same | — | pool's `ASPMembership` | ✅ |
| ASP block-list | `CCWU7V5HXV6NKPJ3UEZMTGFWK3V35YO42AQTPSXPN2GSDK3RXLICNLKT` | same | — | pool's `ASPNonMembership` | ✅ |
| Policy / GVK | `allowlist` + `blocklist`, `traceable`, `gvkAuthorityPubKey` | same | — | `PolicyFlags = 3`, `GvkMode = 2`, `AdminViewKey` equals the published key | ✅ |
| KDF domain | `Lendara Shield` | same | — | same | ✅ |
| Admin | operator `GCCWXZJTF7XMEISC2CXLO6SWETJHE4YBBNVXSMRYOWH7RUOEN2YGGVTE` | — | — | `get_admin` of the pool and both ASP contracts | ✅ |
| Trustline | — | — | — | intake has an authorized USDC trustline; issuer has `auth_required = false` | ✅ |
| Ramp mode | offered | `enabled: true` | SEP-24 deposit enabled, SEP-10 works | — | ✅ |
| Direct mode | offered | `enabled: true` | n/a | — | ✅ |

The web bundle points to `https://lendara-shield.duckdns.org` (API) and `https://soroban-testnet.stellar.org` (RPC); no `localhost` URLs.

## 3. Trustline with a new account

**Pass criterion:** the trustline appears in the account's on-chain state and the system correctly detects whether it exists.

| Step | Result |
|---|---|
| New account (friendbot) | `GDMSOZPN7R4HKQOTDKA52HSB4AAMNNPJIEOFPMN43VPYZN6GQOJ3KOW7`: 10,000 XLM, 0 subentries, no USDC |
| Before the trustline | SAC `balance` fails with `Error(Contract, #13)` "trustline entry is missing"; RPC `getLedgerEntries` returns no entry |
| `change_trust` USDC:`GBVIQ7QV…H47B5` (signed with the Stellar CLI) | [`71981e00…`](https://stellar.expert/explorer/testnet/tx/71981e00b38b8e6738074184585a01156b7b397bd6ecc71a5710c9fa5e8fce7b), successful, ledger 5437699, fee 100 stroops |
| After | 1 subentry, USDC from the right issuer, balance 0, authorized; SAC `balance` returns `"0"`; RPC returns the entry |

**Result:** on-chain, passed. Detection was missing in the system (nothing checked the trustline; a direct deposit failed only at simulation). Fixed in #37 and deployed (`eba56dd`): direct deposit and withdraw read the trustline from the RPC and block the form with instructions when it is missing. **Pending:** signing the trustline with Freighter and checking the notice by hand on the live web.

## 4. Errors and recovery

**Pass criterion:** errors are shown correctly, no operation is duplicated and no state is left inconsistent.

| Scenario | What was checked | Result |
|---|---|---|
| Signature declined | Web tests: declining in onboarding or on an intent calls nothing. A pool operation declined in Freighter returns `wallet request rejected by user` and is not retried | Nothing sent ✅; message was raw English → fixed in #41 |
| Missing trustline | Fixed in #37 and #39 | ✅ |
| Insufficient balance | No pre-check. Short private balance: SDK says `transaction planning rejected: no combination possible`. Short public USDC on a direct deposit: SAC `Error(Contract, #10) resulting balance is not within the allowed range` (simulated from an account with 0 USDC) | Nothing sent ✅; unreadable message → fixed in #41 (translated, and send/withdraw disabled above the synced balance) |
| Expired session | Live API: `issued_at` 10 min old or 2 min ahead → `400 request_expired`; signature over another amount → `400 invalid_signature`; account not onboarded → `404 user_not_found`; none created an intent. The web signs on each click | ✅ (a Freighter account switch with the page open is not detected: follow-up) |
| Failed transaction | Worker tests (CI): ambiguous submit, dropped, refused, failed on-chain, chaotic network, RPC lag, user blocked midway; each asserts exactly one deposit and one transfer | ✅; a partial failure hid the sent steps in the web → fixed in #41 |
| Retries | Worker: backoff and a cap, then manual review. Web: retries only a proving timeout, once, when nothing was sent; buttons are disabled while busy | ✅ |
| No duplicates on-chain | Whole intake history: 6 anchor payments, each followed by exactly 1 pool deposit and 1 private transfer; no failed transactions; USDC balance 0 | ✅ |

**Result:** backend passed. The web passed after #41; the screens were not exercised with Freighter.

## 5. API, database and regression

**Pass criterion:** the tests pass, there are no new errors and the on-chain checks can be repeated.

Run on `develop` at `9bb595d` (the deployed commit).

| Suite | Result |
|---|---|
| `make test-api` against PostgreSQL 17 (temporary local cluster, same major as CI) | 268 passed, 0 failed, 3 ignored (the testnet tests below). The 99 `#[sqlx::test]` tests ran against it (sqlx created its `_sqlx_test` schema) |
| `make test-pool-cli` | 7/7 |
| `make test-js` | 133/133 (web, Vitest) + 2/2 (`api-types`) |
| `make test-web-e2e` | 12/12 (Playwright, Chromium) |
| `make fmt-check`, `make lint` (clippy `-D warnings` on API and pool-cli, ESLint, OpenAPI), `make typecheck`, `make build-js` | Clean |
| `make codegen` | No diff: TS types match `openapi.yaml` |
| `make test-testnet` (on the server, it needs the operator and intake keys) | Passed in all 4 deploys of the day (`185004Z`, `195845Z`, `201106Z`, `202653Z`): the 7 checks of `scripts/verify-testnet.sh` and the 3 `#[ignore]` tests (`testnet_sandbox_starts_an_interactive_deposit`, `testnet_prepares_an_allow_list_insert_without_sending_it`, `testnet_the_intake_is_initialised`) |

**Result:** passed. The check *pool deployment ledger is inside the RPC retention window* will start failing around 2026-11-02 (see Findings).

## 6. Re-verification on the redeployed pool (2026-11-06)

The first pool left the public RPC's history window on 2026-11-02 (Finding 2). It was redeployed on 2026-11-06 at ledger 5,591,375 ([CAB6FI43…KR4](https://stellar.expert/explorer/testnet/contract/CAB6FI43BVMPMAXYCLVZ7R5DYQY3IUJF3G7JMSBEXJHWO7LEUE3H4KR4)), with SPP's bootnode serving events older than the RPC window. Every flow was then run by hand from the live web app with Freighter. All transactions succeeded; in the three ramp transactions the user's address appears 0 times (Horizon: transactions, operations and effects).

| Flow | UTC | Transaction |
| --- | --- | --- |
| Ramp: anchor → omnibus (9.3039215 USDC, intent memo) | 19:08:31 | [7ce42090…](https://stellar.expert/explorer/testnet/tx/7ce4209056f81d311f7a55a2f25e2a99e4ae5cba30ef469e0dd8db5635cf810f) |
| Ramp: pool deposit by Shield | 19:11:11 | [2a8dd765…](https://stellar.expert/explorer/testnet/tx/2a8dd76594d81d566b67cceaa9c7b35b7cd087c952e7d28e6ccb000b2b795bda) |
| Ramp: private transfer to the user's keys | 19:12:11 | [4e3c5c0a…](https://stellar.expert/explorer/testnet/tx/4e3c5c0a1fcfe09df21434252c2d586aea475dc719ec731ba6d38ef3c72c7d9d) |
| Direct: deposit from the user's wallet | 19:01:31 | [e2f171da…](https://stellar.expert/explorer/testnet/tx/e2f171dad9a6f67b3bc0d407fa18beb7e001c5f78f7650d5454f1ab588568aec) |
| Direct: private transfer from the browser | 21:13:11 | [eee163a9…](https://stellar.expert/explorer/testnet/tx/eee163a927aa368798072306ab7d15dfa16c18f1bc318ca0d0962cccf2e6be2d) |
| Direct: withdraw of 5 USDC to the user's account | 21:28:01 | [1a8bb88e…](https://stellar.expert/explorer/testnet/tx/1a8bb88ecbef914ead7a43019e80bbaf154f23cb44f6162caa03df41f1dfc71b) |

A later ramp the same day took about 13 minutes from the anchor's payment to the private transfer. The cause was the public RPC's latency, not Shield or SPP: see `docs/testing/2026-11-06-rpc-latency.md` in the repository.

## Findings

1. **Deposit limit mismatch.** The pool's `MaximumDepositAmount` is `10000000000` stroops (1,000 USDC); the anchor accepts up to 10,000 USDC and the API does not check the pool limit when creating an intent. A ramp intent between 1,000 and 10,000 USDC would be paid by the anchor, refused by the pool and end in `error` with the USDC in the intake. Demo amounts (CRC 5000 ≈ 9.30 USDC) are unaffected. **Resolved for the demo (2026-11-06, #62–#64):** the API publishes the cap as `pool.max_deposit` in `/v1/info`, the deposit page shows it before the user goes to the anchor, and an over-cap payment stops at once in `error` with a clear message.
2. **RPC history window.** The pool was deployed at ledger `5395126`; the public RPC keeps 120,960 ledgers (oldest was `5316399`). At ~5 s per ledger, the deployment leaves the window around **2026-11-02**, after which proving fails in both modes. Options: run SPP's `tools/bootnode` or redeploy (see the repository's AGENTS.md, section 9). **Resolved (2026-11-06):** the pool was redeployed and SPP's bootnode now serves the older history (Section 6).
