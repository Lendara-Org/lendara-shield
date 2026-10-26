# pool-cli

Stateless JSON CLI around the [Stellar Private Payments](https://github.com/NethermindEth/stellar-private-payments) Rust SDK (`=0.2.1`). The Shield API runs it as a subprocess for everything that needs the Groth16 prover: the SDK (`rusqlite`) and the API (`sqlx`) link incompatible versions of the native SQLite library, so they cannot share a binary.

It acts as Shield's **intake** account. Business state, retries and idempotency live in the API; the CLI only prepares, sends and checks transactions.

## Protocol

One command per process. Exactly one JSON object on stdout. Exit `0` on success; exit `1` with `{"error", "message", "retryable"}` on failure. Secrets come from the environment, never from arguments.

| Command | Output |
|---|---|
| `keys` | `{account, note_public_key, encryption_public_key, asp_leaf}` (derives the pool keys once) |
| `balance` | `{stroops}` (private balance in base units, as a string) |
| `prepare-deposit --amount N` | `{hash, envelope_xdr, valid_until, sequence}` |
| `prepare-transfer --amount N --note-key 0x.. --encryption-key 0x..` | same, to the recipient's raw pool keys |
| `submit --envelope XDR` | `{hash}` |
| `status --hash HEX` | `{status: success \| failed \| pending \| not_found, result_xdr}` |
| `audit --limit N` | `{transactions: [{ledger, inputs: [{owner, amount}], outputs: [...]}]}`, newest first; needs `SHIELD_GVK_KEY_FILE`. `owner` is the note key as the SDK's audit prints it (the reverse bytes of the registered key) |

`prepare-*` proves, simulates and signs but does **not** send: the caller stores `hash` first, then calls `submit`. SDK pool transactions have no time bounds (`valid_until: null`); a transaction that is `not_found` can only be replaced once the intake's sequence number has passed `sequence`. Only single-transaction plans are supported (`multi_transaction_plan` otherwise).

## Environment

Same names as the API's `.env` (written by `make setup-testnet`): `SHIELD_INTAKE_SECRET`, `SHIELD_DEPLOYMENT_FILE`, `SHIELD_INTAKE_WALLET_PATH`, `SHIELD_CIRCUITS_LOCK_FILE`, `SHIELD_CIRCUITS_DIR`, and optionally `SOROBAN_RPC_URL`.

## Build and test

```sh
make build-pool-cli   # target/release/pool-cli
make test-pool-cli
```
