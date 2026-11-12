# Automatic wallet sync recovery — 2026-11-11

## Problem and fix

The live ramp test completed its payment, but the first wallet read displayed
an SDK event-response decoding error and required clicking **Sync**. The exact
upstream cause of the malformed response was not established. A fresh browser
later read the full history and displayed the expected balance automatically.

The web now recovers from transient event-network errors automatically:

- The complete SDK balance read is wrapped, including its internal event sync.
- The old explicit `client.sync()` before `balance()` was redundant and left
  the SDK's second sync outside recovery. It has been removed.
- Only errors in the SDK's `indexer: network error:` and
  `indexer catch-up: network error:` contexts are retried, at most twice after
  the initial attempt, with 250/500 ms delays. Concurrent calls to the same
  read share the promise and retry budget.
- The helper also protects the explicit read-only pre-sync before a user
  transfer or withdrawal. It never retries the transfer or withdrawal itself.
- Persistent sync errors still stop after three attempts. The UI explains the
  failure in English/Spanish, preserves the unavailable balance, and leaves
  the manual **Sync** button available. It does not display a fabricated zero.
- There are no new dependencies or API changes. The browser SDK stays 0.2.1.

## Verification

The changed web passes lint, TypeScript and **151 web tests**, plus the two
API-types tests, the production build and all **13 Playwright smoke tests**.
New tests cover successful recovery, shared retry budgets,
the three-attempt cap, terminal/non-network errors, later reads after both
success and failure, and the persistent-error UI.

A browser regression test used the actual SPP 0.2.1 browser SDK, an existing
test account from the live payment, its private OPFS wallet, the deployed API,
and real RPC/history. No new payments were sent. A test signer replaced only
Freighter confirmations. Each scenario ran in its own page because the SDK's
OPFS wallet is owned by its storage worker.

The patched frontend was served from a local production build under the
demo's browser origin using Playwright routing. API and SDK assets remained
real. The only deliberately modified RPC responses were the injected faults:
HTTP 200 JSON-RPC `getEvents` responses with a malformed empty `result`.

| Frontend | Injected faults | Observed result | Manual Sync clicks |
|---|---|---|---|
| Currently deployed, before this fix | One malformed event response | SDK decoding error; no automatic balance | 0 |
| Patched local build | One malformed event response | Automatically recovered **1.4607843 USDC** | 0 |
| Patched local build | Three malformed event responses | Stopped after three failed reads; localized error | 0 |
| Patched local build | None | Automatically displayed **1.4607843 USDC** | 0 |

The deployed frontend therefore reproduces the failure under the same injected
condition that the patched frontend survives. This establishes a regression
check; it is not a claim that the synthetic fault identifies the provider's
original malformed payload or that a permanently unavailable RPC can succeed.

Claude independently reviewed the implementation, bilingual copy and tests.
It found no blocking defects and confirmed that the read retry does not wrap
or nest with transaction-submission retries. Its review did not independently
rerun the browser fault-injection experiment.

## Deployment scope

This is a **web-only change**. It does not include the separately prepared
native SDK/SQLite upgrade or migrate the intake wallet. It remains local until
reviewed, committed, pushed and deployed from the environment that has server
access. After deployment, verify a fresh connection and payment without manual
Sync; retain the previous web build for rollback.

Keep this fix separate from the native SPP/SQLite update when committing.
The intended branch is `fix/web-wallet-sync-recovery`, targeting `develop`,
with commit title `fix(web): recover transient wallet event sync failures`.
Include only the web changes, this report and the balance-read paragraph in
AGENTS.md §6.8. No API, pool CLI, lockfile or `.env.example` change is needed
for this web fix.
