# Scenarios

Every outcome the demo can show, and what Shield does with it. There are two independent gates:

1. **Shield's KYC** (the pool's ASP), chosen in `/onboard`. Sandbox only.
2. **The anchor's outcome** (ramp mode), chosen in the anchor's widget with a test card. These are Lendara sandbox's own scenarios; a real anchor runs real KYC.

## 1. Shield's KYC (`POST /v1/onboarding`, field `scenario`)

Every onboarding request carries the account's SEP-53 signature over its keys; a missing or foreign signature answers `400` and nothing is registered.

| Scenario | KYC | Allow-list (ASP) | What happens next |
|---|---|---|---|
| `approved` (default) | approved | Leaf inserted on-chain → `allowed` | Can deposit (both modes), transfer and withdraw |
| `rejected` | rejected | Never inserted → `not_allowed` | The pool refuses the user's proofs; `POST /v1/intents` answers `409 user_not_allowed`; the web sends the user back to onboarding |
| `blocked` | approved | Inserted, then the note key is added to the **block-list** → `blocked` | Frozen: every proof needs non-membership in the block-list, so nothing moves until the ASP unblocks it |

If the chain is unavailable during onboarding, the user stays `pending` (`201`). Reading the status (`GET /v1/users/{account}`) settles an ASP write already in flight, but never sends one, so a write that could not even be prepared waits for the next signed onboarding call. After 20 seconds in `pending` the web offers *Retry the registration*, which signs again and resumes it (idempotent: same account and keys).

Admin actions (`ADMIN_TOKEN`): `POST /v1/asp/block` and `POST /v1/asp/unblock`. The allow-list is append-only (SPP's membership contract has no removal), so revoking a user means blocking their key.

Blocking a user mid-intent:

- `POST /v1/intents` checks the allow-list, with the user's row locked, so a block confirmed at the same time either refuses the new intent or rejects it. If the block lands while the anchor is answering, the request answers `409 user_not_allowed` and the user never gets the interactive URL.
- When the block is confirmed, the user's intents that hold no funds yet (`created`, `awaiting_anchor`) move to `rejected`. If the anchor still pays one of them, the payment is kept as unmatched for manual review.
- Once the money is in Shield's custody (`funds_received` and later), an intent only ends `completed` or `error`, never `rejected` or `expired`. The shielding worker checks the user before the deposit, before the private transfer and before rebuilding a transfer that can no longer land: a blocked user's intent ends in `error` with the funds in Shield's custody (the omnibus account, or Shield's own note after the deposit). Nothing is sent to the frozen key. A transfer already sent is not chased: whatever a blocked key receives stays frozen, because spending needs a proof that the key is not on the block-list.
- Unblocking does not revive rejected intents.

## 2. The anchor's outcome (ramp mode, Lendara sandbox widget)

Any future expiry date and any 3-digit CVC work.

| Test card | Sandbox outcome | Intent in Shield | Pool |
|---|---|---|---|
| `4242 4242 4242 4242` | Approved: USDC paid to Shield's omnibus account with the intent's memo | `awaiting_anchor → funds_received → shielding → transferring → completed`, with three hashes | Deposit from the omnibus account + private transfer to the user's keys |
| `4000 0000 0000 0002` | Rejected (KYC) | `error`, with the anchor's message | Untouched |
| `4000 0000 0000 9995` | Payment not received | `expired` | Untouched |
| `4000 0000 0000 0077` | Approved after 30 s | Stays `awaiting_anchor` while the anchor is `pending_anchor`, then completes | As approved |

Shield follows the anchor with `GET /sep24/transaction` (the sandbox has no webhooks) and detects the payment on Horizon by memo, asset code **and** issuer.

## 3. Money edge cases (covered by tests)

| Case | Behaviour |
|---|---|
| Someone sends the asset with an intent's memo (front-running the anchor) | Recorded as "matches an intent's memo; not confirmed by the anchor"; only the payment the anchor reports funds the intent (verified on testnet: dust [`1f533b4e…`](https://stellar.expert/explorer/testnet/tx/1f533b4e04b51d2734162b4a2730d85cecbf9c796cfac47430157ba426c1f178) stayed in review, the anchor's [`3a971137…`](https://stellar.expert/explorer/testnet/tx/3a97113724334b86130aeb18d0de249f7090dc8364da2ca4cb30d1017883c3b6) completed the intent) |
| The anchor reports `completed` before Shield has read its payment on Horizon | Waits and checks again a few seconds later |
| The anchor reports `completed` but its payment never shows up (no memo, wrong asset, no `stellar_transaction_id`) | Waits 15 minutes from the first report, then `error` for manual review; a payment arriving later is kept as unmatched |
| Someone else asks for a deposit in a user's name | Refused: `POST /v1/intents` needs the account's SEP-53 signature (`400 invalid_signature`) |
| A signed intent request is sent twice | The second answers `409 request_replayed`; the anchor is asked once |
| A client floods onboarding or intents | `429 rate_limited` past `RATE_LIMIT_PER_MINUTE` per client IP |
| A payment arrives after the intent expired, or a second payment arrives | Stored as unmatched in `intake_payments`: money in custody for manual review; it never funds anything automatically |
| Wrong asset or wrong issuer | Not matched |
| The pool submission times out but the transaction landed | The stored hash is checked first; it is never paid twice |
| The transaction was dropped | The **same** signed envelope is resent |
| The RPC has not indexed a landed transaction yet | Not replaced until it stays unseen for 60 s after the omnibus account's sequence passed it |
| The proof went stale (e.g. the ASP root changed) | Prepared again and replaced; retries back off exponentially |
| Repeated failures | `error` with "the funds are in Shield's custody and need review" |
