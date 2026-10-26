# Shield Protocol — draft 0.1

> Status: **draft**, written for HackMeridian 2026. Level 1 is implemented by Lendara Shield on testnet. Level 2 is a proposal and is **not** implemented.

The Shield Protocol lets fiat on-ramps deliver money **into a privacy pool** instead of to a user's public address, while keeping KYC, allow-listing and audit in the hands of regulated parties. It is named without the "Lendara" brand so any anchor can adopt it.

**Building blocks:** [SEP-1](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0001.md), [SEP-10](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md), [SEP-24](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0024.md), and a [Stellar Private Payments](https://developers.stellar.org/docs/build/apps/privacy) pool with an ASP (allow-list + block-list) and a Global View Key.

## Roles

| Role | Does |
|---|---|
| **User** | Holds a Stellar wallet; derives the pool keys in the wallet (spending keys never leave it) |
| **Anchor** | SEP-24 on-ramp: KYC, takes the fiat payment, pays the asset |
| **ASP** | Runs (or relies on) KYC and maintains the pool's allow-list and block-list |
| **Pool operator** | Deploys and administers the pool |
| **Auditor** | Holds the pool's Global View Key; can decrypt every note |
| **Shield** (level 1) | ASP + omnibus account + SEP-24 client in front of any anchor |

## Level 1 — Shield as middleware (implemented)

Shield sits between **any** SEP-24 anchor and the pool. The anchor is not modified.

### Onboarding

1. The user's wallet derives the pool keys (note key and encryption key) and the ASP membership leaf `leaf = H(note_public_key, membership_blinding)`.
2. The user sends **only** `{stellar_account, note_public_key, encryption_public_key, asp_leaf}` to Shield. The user does **not** register in the pool's public key registry: that is a public transaction from their own address.
3. Shield runs KYC. On approval, Shield (as ASP) inserts `asp_leaf` into the allow-list on-chain. Blocking inserts the note key into the block-list. The allow-list is append-only in SPP.

### Deposit (one intent)

1. The user asks Shield to deposit `amount` of `fiat_currency`.
2. Shield authenticates with the anchor using **SEP-10**, as its omnibus account with an `id` memo that identifies the user (each user is a distinct customer for the anchor's KYC). Shield verifies the challenge fully before signing it.
3. Shield calls `POST /transactions/deposit/interactive` (**SEP-24**) with:
   - `asset_code`: the pool's asset
   - `account`: **Shield's omnibus account**
   - `memo_type: id`, `memo`: a number that identifies this intent
   - `source_asset: iso4217:<fiat_currency>`
4. The user completes the anchor's interactive flow. The anchor pays the omnibus account with that memo.
5. Shield detects the payment (Horizon), matches it by memo, **asset code and issuer**, and records the amount actually received.
6. Shield deposits that amount into the pool from the omnibus account (`ext_amount > 0`). The note belongs to Shield.
7. Shield transfers it privately to the user's registered keys (`ext_amount = 0`).

**Idempotency requirement:** each pool transaction is proved, signed and its hash **persisted before submission**. On retry the implementation checks the stored hash and resends the same envelope. It may prepare a replacement only when the original can no longer land, that is, when its sequence number has been consumed and the hash is still unknown after a grace period that covers RPC indexing lag.

**State machine:** `created → awaiting_anchor → funds_received → shielding → transferring → completed`. The error exits are `rejected`, `expired` and `error`. Once funds are in custody, an intent can only end `completed` or `error`.

### Audit

- The auditor decrypts pool activity with the Global View Key. The ASP maps note keys to its users.
- An implementation **must not** expose the view key to browsers, and **should** give it only to the process that audits.

### What level 1 achieves

On-chain, the deposit path shows "anchor → omnibus" and "omnibus → pool". The user's address appears in neither. See the [threat model](../threat-model.md) for amount/timing correlation and custody.

## Level 2 — Native shielded deposits (proposal)

The anchor deposits **directly into the pool** to the user's keys, which removes Shield's custody step. This is an extension of SEP-24.

### Discovery (SEP-1 `stellar.toml`)

```toml
SHIELD_PROTOCOL_VERSION = "0.1"
SHIELD_POOL_CONTRACT = "C…"        # SPP pool for the asset
SHIELD_ASP_ENDPOINT = "https://…"  # where wallets onboard keys (may be the anchor itself)
```

### Capability (SEP-24 `GET /info`)

```json
{
  "deposit": { "USDC": { "enabled": true } },
  "features": { "account_creation": true, "claimable_balances": true, "shielded_deposit": true }
}
```

### Interactive deposit (SEP-24)

`POST /transactions/deposit/interactive` accepts, **instead of** `account`:

| Field | Meaning |
|---|---|
| `destination_type` | `"shielded"` |
| `shielded_recipient` | The recipient's pool public keys, `<note_public_key>:<encryption_public_key>` (`0x` hex) |

The SEP-10 session still identifies the customer (KYC). The anchor must reject a `shielded_recipient` whose leaf is not in the pool's allow-list (it is, or works with, the ASP).

### Settlement

The anchor's distribution account calls the pool's deposit with an output note to `shielded_recipient`. It reports the result through `GET /transaction` as usual:

```json
{
  "transaction": {
    "id": "…",
    "status": "completed",
    "stellar_transaction_id": "<pool deposit hash>",
    "shielded": { "pool_contract": "C…", "ext_amount": "9.3039215" }
  }
}
```

### Withdraw (outline)

The user withdraws from the pool to the anchor's account with a valid membership proof (SEP-24 withdraw, `ext_amount < 0`), and the anchor pays out fiat. Details to be specified.

### Security considerations

- **Correlation.** A deposit's public amount equals the fiat quote. Anchors should batch deposits, round amounts and add delays (the same mitigations apply at level 1).
- **Custody.** Level 2 removes the omnibus custody of level 1. The anchor never holds the note.
- **Separation of duties.** The ASP's signing key, the anchor's distribution key and the auditor's view key should belong to different parties or at least different keys.
- **Issuer powers.** A classic asset whose issuer can revoke or claw back can freeze the pool's balance. Pools should prefer assets with `AUTH_IMMUTABLE` or document the risk.
- **Who signs is public.** Users' own pool operations reveal their account as the signer unless a relayer (fee-bump) submits them.

## Open questions

1. Should `shielded_recipient` be a registry lookup (by `G…` address) or raw keys? Raw keys avoid the public registry transaction; a registry is easier for peer-to-peer payments.
2. How does an anchor prove to a wallet that its deposit went to the right keys without revealing them publicly? (Per-note selective disclosure is a candidate.)
3. ASP federation: can several anchors share one pool's allow-list?
