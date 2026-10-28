# Testnet verification — 2026-10-28

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

- The 8 transactions listed in the README's *Proof on testnet* are successful on Horizon.
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

## Findings

1. **Deposit limit mismatch.** The pool's `MaximumDepositAmount` is `10000000000` stroops (1,000 USDC); the anchor accepts up to 10,000 USDC and the API does not check the pool limit when creating an intent. A ramp intent between 1,000 and 10,000 USDC would be paid by the anchor, refused by the pool and end in `error` with the USDC in the intake. Demo amounts (CRC 5000 ≈ 9.30 USDC) are unaffected.
2. **RPC history window.** The pool was deployed at ledger `5395126`; the public RPC keeps 120,960 ledgers (oldest was `5316399`). At ~5 s per ledger, the deployment leaves the window around **2026-11-02**, after which proving fails in both modes. Options: run SPP's `tools/bootnode` or redeploy (AGENTS.md §9).
