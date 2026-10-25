# Spike H1: Stellar Private Payments (SPP)

- **Date:** 2026-10-25
- **Timebox:** 90 minutes
- **SPP version:** tag `sdk-v0.2.1` = `npm-v0.2.1` = commit [`30d8af7`](https://github.com/NethermindEth/stellar-private-payments/commit/30d8af7de55302c48feef428168a0c5f25e84f18) (2026-10-16). `main` was at `9bc3a78` (2026-10-23); we pin the tagged release so contracts, Rust crate and npm package match.
- **Code:** [`spikes/spp-e2e`](../../spikes/spp-e2e) — standalone crate, not part of the API.

## Questions and answers

| # | Question | Answer | Evidence |
|---|---|---|---|
| a | Can we deploy our own pool for an arbitrary SEP-41 asset, with allow-list and our own admin? | **Yes.** `deployments/scripts/deploy.sh` takes `--pool allowlist-blocklist:gvk-traceable:classic:<CODE>:<ISSUER>:<SAC>` and `--admin`. Verifying keys for every policy/GVK combination are committed under `deployments/testnet/circuit_keys/`. | `DEPLOY.md`, `deploy.sh --help` |
| b | Does the native Rust SDK work server-side with our own signer? | **Yes.** Crate `stellar-private-payments = 0.2.1`: `LocalSigner::new(secret, passphrase, address)`, `Account::derive_privacy_keys()`, `PrivatePool::{deposit, transfer, withdraw}`. Async and blocking APIs. | `sdk/native/README.md`, spike run below |
| c | How does the public key registry work? | Maps a `G...` address to its note and encryption public keys. `Account::register_public_keys()` registers; `PrivatePool::transfer` accepts the recipient's `G...` and resolves it in the registry. | `examples/transfer.rs` |
| d | Web SDK in Next.js | npm `stellar-private-payments`: use dist-tag **`latest` (0.2.1)**. The `alpha` tag is stale (`0.1.0-alpha.1`). Exposes `deriveAspUserLeaf`. To be exercised by the web PR (H7). | `npm view stellar-private-payments dist-tags` |
| e | Can the intake account hold its own pool keys server-side, without Freighter? | **Yes.** Same as (b): keys are derived from a signature by the account's own secret and stored in the SDK's SQLite wallet. | spike run below |
| f | Does the Lendara sandbox accept a memo in SEP-10 and SEP-24 towards another account? | **Yes.** SEP-10 challenge accepts an `id` memo (`internal/sep10/sep10.go`). SEP-24 deposit accepts `account` + `memo` + `memo_type` (`text`, `id`, `hash`) and the payment carries it (`internal/handlers/sep24.go`, `internal/payment/processor.go`). | sandbox source |

## Findings that change the design

1. **The allow-list is append-only.** `asp-membership` only has `insert_leaf`; there is no removal. Blocking a key means inserting it in the **block-list** (`asp-non-membership`, which has `insert_leaf` and `delete_leaf`). Shield's pool must therefore be `allowlist-blocklist`.
2. **Shield cannot compute a user's ASP leaf on its own.** `leaf = H(note_public_key, membership_blinding)` and the blinding is a secret derived in the user's wallet. The web computes `deriveAspUserLeaf()` and sends the leaf with `POST /v1/onboarding`.
3. **The SDK has no ASP admin calls.** Inserting leaves is a plain contract invocation by the pool admin. H5 needs an `AspContract` adapter (Soroban `invokeHostFunction` signed by the operator).
4. **The Global View Key is not a Stellar key.** It is a Baby JubJub keypair generated with `tools/gvkey-gen`; its public half is fixed at deploy time. `SHIELD_AUDITOR_SECRET` becomes the path to (or JSON of) that keypair. Auditing uses `gvk::GvkAudit`, which is async-only.
5. **Idempotent submission is possible.** Besides one-shot `deposit`/`transfer`, the SDK exposes `prepare_* → prove_next → sign → submit → confirm`. The worker can persist the signed transaction hash **before** `submit` and, on retry, `confirm(hash)` instead of resubmitting (CLAUDE.md §0.6).
6. **Memo strategy (D4):** SEP-24 memo of type `id` from a dedicated `bigint` column in `intents` (a UUID does not fit in a 28-byte `text` memo). The SEP-10 memo should identify the **user** (each user is a distinct customer for the anchor's KYC), while the SEP-24 memo identifies the **intent**.
7. **No bootnode needed for the hackathon.** Proving needs the pool's full event history and the public RPC keeps 7 days. A pool deployed now stays inside the window through Meridian (28–29 Oct). Past that, run SPP's `tools/bootnode` or redeploy.
8. **Registering in the public key registry exposes the user's `G...` address.** `register_public_keys` is a public transaction sent by the user's own account, so anyone can see that the address is a pool user (not amounts or counterparties). Since Shield already stores the user ↔ key relation as the ASP, the ramp flow transfers to the user's **raw keys** (`TransferRecipient::keys(note_public_key, encryption_public_key)`) and the user never registers on-chain. That SDK path exists (`examples/transfer.rs`) but this run transferred by address; H6b must verify it with a real hash. The registry stays optional for direct-mode peer-to-peer payments.
9. **Who submits is visible.** A pool transaction's source account is the signer. In ramp mode that is Shield's intake (fine); in direct mode the user's address shows as having used the pool for every transfer, until the relayer (P3) exists.
10. **Classic issuer powers.** `deploy.sh` warns when the asset issuer has `AUTH_REVOCABLE`, `AUTH_CLAWBACK_ENABLED` or lacks `AUTH_IMMUTABLE`: the issuer could freeze or claw back the whole pool. Check the sandbox USDC issuer flags in H4 and document it in the threat model.

## Decisions

| # | Decision | Outcome |
|---|---|---|
| D1 | Backend language | **Rust (Axum)** with `stellar-private-payments = "=0.2.1"`. Plan B (Go + Node sidecar) discarded. |
| D2 | Own pool or existing | **Plan A: own pool**, `allowlist-blocklist` + `gvk-traceable`, Shield's operator as admin. The spike used native XLM; H4 deploys the same configuration for the sandbox USDC (needs its SAC). |
| D3 | Recipient identification | **Raw pool keys** sent by the web at onboarding (finding 8), not the on-chain registry. |
| D4 | Memo type towards the anchor | `id` (see finding 6) |
| D6 | Fallback if the sandbox rejects memos | Not needed (see f) |

## Testnet run

Spike deployment (throwaway; H4 deploys Shield's real one). `deploy.sh` from tag `sdk-v0.2.1`:

```sh
deployments/scripts/deploy.sh testnet --deployer shield-spike-operator \
  --asp-levels 10 --pool-levels 20 --max-deposit 1000000000 --kdf-domain "Lendara Shield" \
  --pool allowlist-blocklist:gvk-traceable:native:$(stellar contract id asset --asset native --network testnet) \
  --gvk-authority-pubkey-file admin-pub.json
```

| Contract | Id |
|---|---|
| Pool (XLM, AB, GVK traceable) | `CCAEA4MKORUJV3G3V47SKWLJKQZBYGGFU42JSCXNUIJIZMNN2MSDLFMO` |
| ASP membership | `CBXCTA27XVLULCT4O5LCE2ETKW7A6PXEOFXOKHHFP4JCPRT72T7HBQLL` |
| ASP non-membership | `CBGH7D7JZ3FRJ6ILEDRABBUWTEEV42CKBWTPQQWMHNF7H3OXWREEOAFA` |
| Public key registry | `CDKEJFD2TJGJ5TL2COAA6ULRDCO55EPBCYB2LZD2QKSR4MAO6NPRRMWL` |
| Verifier `AB_gvk_T` | `CCQWHSMUF35HFOLUV5PTUVDMV55MTIZM7CD66AEC5KWPGHINGHACFC7Z` |

Accounts: operator/ASP admin `GAABX6R4…WWMS`, alice (plays the intake) `GDZOAGBQ…6XZC`, bob (plays the user) `GBJ3H74T…PFWY`.

| Step | Result | Hash |
|---|---|---|
| alice registers keys | ok | [`9858a411…`](https://stellar.expert/explorer/testnet/tx/9858a411f994187d480f0f1a27cc4bfe5865de6d97405f2cfebe3b1f9ef0407a) |
| bob registers keys | ok | [`2f467186…`](https://stellar.expert/explorer/testnet/tx/2f4671864a133cd6aa5527b8d7ab549d4f7f2bc8e8ff970d8e6fd97bc58be24b) |
| **Gatekeeper:** alice deposits before being allow-listed | **rejected** by the SDK (`ASP membership sync required: RegisterAtASP`), nothing submitted | — |
| ASP admin inserts alice's leaf | ok | [`a51d5ab3…`](https://stellar.expert/explorer/testnet/tx/a51d5ab30ca627968f81b38fd00750ac885a5b613ea98baafcda1b24bf817a50) |
| ASP admin inserts bob's leaf | ok | [`7932974c…`](https://stellar.expert/explorer/testnet/tx/7932974c04b25deb56892259191a89a26dbb18dac6f6be67031d1639b0cb663c) |
| **Deposit** 2 XLM by alice (`ext_amount > 0`) | ok, 16 s incl. proving | [`6a8bce6e…`](https://stellar.expert/explorer/testnet/tx/6a8bce6e806d037d4ed6f95aac536a5049791772a67d9549c4ef606b1b1e67d3) |
| **Private transfer** 1 XLM alice → bob (`ext_amount = 0`) | ok; alice 1 XLM, bob 1 XLM | [`770f0c79…`](https://stellar.expert/explorer/testnet/tx/770f0c7954347bfd2663f0c7b4b268c05bcfdc5205f702253adf60e0498b17b6) |
| **Withdraw** 0.5 XLM by bob (`ext_amount < 0`) | ok; bob 0.5 XLM | [`780f464f…`](https://stellar.expert/explorer/testnet/tx/780f464fb42eedfd212d44285a6749af6bf1891ebe2679d5fca18ae78baff72c) |

**Privacy check.** The transfer envelope contains bob's address **0 times**; its source account is alice. Bob's Horizon history shows only Friendbot, his registration and his withdrawal: the transfer to him is not linked to his address.

**Audit check.** `spp-e2e audit` decrypts all three pool transactions with the Global View Key: owner note key and amount of every real input and output (e.g. the transfer: input 2 XLM from alice's key, outputs 1 XLM to bob's key and 1 XLM change). The `undecryptable` slots are the zero-value dummy notes padding the 2-in/2-out circuit.

### Reproduce

```sh
cd spikes/spp-e2e && cargo build --release
export SPIKE_DEPLOYMENT=.../deployments/testnet/deployments.json   # written by deploy.sh
export SPIKE_CIRCUITS_JSON=.../deployments/testnet/circuits.json
export SPIKE_CIRCUITS_DIR=/tmp/spp-circuits
export SPIKE_SECRET=$(stellar keys secret <alias>) SPIKE_WALLET=<alias>.sqlite
target/release/spp-e2e onboard            # prints the ASP leaf
stellar contract invoke --id <ASP_MEMBERSHIP> --source <admin> --network testnet -- insert_leaf --leaf <leaf as decimal>
target/release/spp-e2e deposit 20000000
target/release/spp-e2e transfer <G...> 10000000
target/release/spp-e2e withdraw 5000000
SPIKE_GVK_KEY_FILE=admin-d.json target/release/spp-e2e audit
```
