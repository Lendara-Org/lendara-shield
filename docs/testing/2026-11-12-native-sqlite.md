# Native SQLite security update — 2026-11-12

## Reason and scope

The web wallet recovery PR's full CI exposed an existing native dependency
failure: `libsqlite3-sys 0.38.2` is affected by
[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459.html).
The advisory concerns opening a manipulated database/WAL pair. No advisory
exception is added and no security check is disabled.

Only the native `pool-cli` dependency moves to SPP `=0.3.0`, resolving
`rusqlite 0.41.1` and `libsqlite3-sys 0.39.1` (SQLite 3.54.1).
`default-features = false` and `unencrypted-storage` retain plaintext storage;
SDK 0.3.0's default encrypted-storage feature is intentionally not enabled.
The adapter's production transaction code and the API state machine are unchanged.
Browser SDK, contracts, pool, circuit lock, environment variables and HTTP/CLI
contracts stay unchanged.

> **Deployed.** This change was deployed to the demo server on 2026-11-12 and
> the live intake wallet is now on schema v7: never point SDK 0.2.1 at it.
> The cutover, backup and live ramp check are recorded in
> [2026-11-12-native-deploy.md](2026-11-12-native-deploy.md). It reached `main`
> in the release PR #89.

## Compatibility verification

`tests/fixtures/legacy-v5.sqlite3` is synthetic, generated with the five exact
SQL migrations from the published SDK 0.2.1. It contains no real account or
spending authority: key fields use distinct asymmetric test patterns that are
deliberately not matching keypairs.
The adjacent Python generator recreates its schema and synthetic content from
that published crate; byte-for-byte SQLite output is not promised.

The new tests exercise the actual SDK storage layer and check:

- Migration from schema v5 to v7, and repeated reopen of the resulting wallet.
- Preservation of private/public key bytes, membership blinding, exact unspent
  amount, both spent and unspent notes, sync cursor/ledgers and every operation field.
- A forked storage connection sees the same spendable amount.
- Storage remains plaintext and does not create an encryption-key sidecar.
- Unreadable files, too-new schemas and failed foreign-key migrations are
  rejected without replacing their contents or advancing their schema.
- The migration clears event-processing marks for the SDK's historical replay.

Local validation: native format, strict Clippy, tests, cargo-deny and release
build passed; the expanded suite has 15 native tests. The new release binary also queried the
existing testnet private-transfer hash
[`098aadfb…28cb3`](https://stellar.expert/explorer/testnet/tx/098aadfbadbfb70d45241cd8efefee6627b75c0b9fbf94cd701c8dfecfd28cb3)
and returned `success`, without opening a wallet or submitting anything.
A separate Python ledger probe received HTTP 403; the actual SDK status
query against the same public SDF RPC succeeded.
Existing API shielding-worker tests in CI continue to enforce exactly
one deposit/transfer across retries and ambiguous submissions; the update does
not add transaction retries to the CLI. The fixture tests do not prove
compatibility with every possible production database or replace a staging
test against a consistent copy of the actual intake wallet.

The SDK 0.3.0 circuit manifest adds a key-ownership circuit, but the existing
policy transaction circuit fingerprints are unchanged from SDK 0.2.1. Shield's
CLI does not call the changed public-key registration flow. Keep using the
existing deployment's circuit lock rather than substituting SDK 0.3.0's new
manifest during native deployment.

## Required native deployment and rollback procedure

This is the procedure the 2026-11-12 deployment followed; reuse it for any
future native SDK upgrade or wallet restore:

1. Stop the API, including shielding, auditing and its startup readiness check;
   do not run `init-intake`, `setup-testnet.sh` or the deploy script before the
   backup. These also open and migrate the wallet. Keep the API's persisted
   transaction hashes/statuses available for reconciliation.
2. Make a consistent private backup of the wallet and any `-wal`/`-shm` sidecars,
   and retain the previous binary and configuration. Never copy wallet secrets
   into a repository or test fixture.
3. Run integrity, key, note and balance comparisons on a separate copy with the
   new binary. Confirm sync through the current RPC and bootnode, and prepare
   and simulate against the deployed contracts before enabling live writes.
4. Upgrade and complete the first historical replay under exclusive wallet
   ownership, before the worker's 180 s budget applies. Then run a controlled
   testnet flow and check the persisted hashes before resuming normal work.
5. If reverting, stop consumers again and restore both the old binary and its
   matching pre-upgrade wallet backup. SDK 0.2.1 cannot open schema v7. Reconcile
   transactions with the API/network and replay chain events before resuming;
   never blindly resend an operation after restoring a wallet snapshot.

Encryption of storage remains separate work.
