# Exhaustive audit and fixes (2026-11-06/07)

An end-to-end test of the whole product (code, live demo, testnet) by Claude, with an independent audit by Codex. Every problem found was fixed in its own PR with tests and a second Codex review, then deployed and verified live.

## What was tested

| Area | How | Result |
|---|---|---|
| Automated suites | `make fmt-check lint typecheck test deny build test-web-e2e coverage` | Rust 299, web 143, Playwright 13 passing; coverage 90.6% |
| Testnet checks | `make test-testnet` on the server | All passing (accounts, trustline, SAC, pool config, SPP verification, bootnode, sandbox, intake initialised) |
| Live API scenarios | Fresh testnet users against the demo | 16 of 16: onboarding rejected, ramp approved (balance checked), anchor KYC rejected, payment not received, delayed approval, blocked user, replayed request, expired request, forged signature |
| Browser | Real Chromium, the SPP browser SDK, the anchor sandbox; a stand-in answers Freighter's signing requests | Onboarding, ramp, private transfer, withdraw (after a trustline), direct deposit; recipient balance exact |
| Admin and audit | On the server, tokens never leaving it | Block, block again (409), unblock, unknown user (404), wrong token (401); GVK audit attributes intake and user notes |
| Black box (Codex) | 49 hostile requests, all pages, headers, on-chain cross-checks | No 500s; pool, token and cap on-chain match `/v1/info`; relayer claim confirmed against SPP `sdk-v0.2.1` |

## Findings and fixes

| # | Severity | Finding | Fix |
|---|---|---|---|
| 1 | High (conditional) | Funding an intent selected the anchor's payment by hash and memo only: a payment of another asset (or a "USDC" from another issuer) could fund it if the anchor reported that transaction | #66: funding also requires the pool's asset code and issuer. No production intent was affected |
| 2 | Medium | A status change and its `intent_events` row were two autocommitted statements on most callers | #67: one transaction (a savepoint inside the caller's) |
| 3 | Medium | An intent creation cut between the database and the anchor stayed `created` forever | #68: each anchor round ends intents left `created` for 5 minutes; migration 0010 indexes the lookup |
| 4 | Medium | No database checks on event statuses, or on a funded intent having its amount and payment | #67: migration 0009 |
| 5 | Low | Errors produced around the handlers (405, an undecodable path, timeouts) did not use `{error, message}`; the contract missed some codes | #70: a response layer rewrites them; `openapi.yaml` lists 405/408/500, `invalid_limit`, `invalid_tx_hash` and the real amount rules |
| 6 | Low | No HSTS; old unprefixed redirects without security headers | #71: `deploy/Caddyfile` (now versioned) sets HSTS and baseline headers at the edge |
| 7 | Low | The GVK audit answered 408 while the worker was shielding (shared wallet lock, 15 s cut) | #69: its own timeout, one audit at a time (`409 audit_in_progress`), a "decrypting" notice on `/audit` |

Each code fix (#66–#70) includes a regression test. Before each pair was merged (#66/#67, #68/#69, #70/#71), Codex ran the Rust suite itself per PR and with both merged (the web suite too, on the merged #68/#69 and #70/#71), and reverted every fix to confirm its tests fail without it. #71 changes the proxy, not code: it was validated with `caddy validate` on the server and checked live with `curl` after the deploy.

## The pool's deposit cap: resolved for the demo

The pool accepts at most **1,000 USDC per deposit** (fixed at deployment), while the sandbox anchor accepts up to 10,000 USDC. Since #62–#64:

- the API reads the cap and publishes it as `pool.max_deposit` in `/v1/info`;
- the deposit page states it under the amount, before the user goes to the anchor;
- an over-cap payment stops at once in `error` with a clear message (no wasted retries) and the USDC stays in the intake for review. This relies on the API reading the cap at startup; if that read fails, `max_deposit` is `null`, the page shows no cap, and the pool still refuses the deposit after the usual retries (same custody outcome);
- `docs/scenarios.md`, the web docs and AGENTS.md §9 explain it (about CRC 684,000 at the sandbox's quote on 2026-11-06).

A reviewer would have to type a very large amount to reach it. **Future improvement, not a blocker:** split an over-cap payment into several deposits of at most the cap, then transfer privately (a fit for the SCF deliverable on resolving funds in custody). It changes the worker and the schema (one intent, several pool transactions, each idempotent), so it is planned after the submission.

## Not problems

- **A 404 in the browser console on onboarding.** The web asks `GET /v1/users/{account}` before a new user is registered; `404 user_not_found` is how it knows to show the form. Browsers log every 404.
- **`GET /bootnode` answers 405.** SPP's bootnode only accepts JSON-RPC over POST.

## Open observation

During the run, one private transfer from the browser stalled for 10 minutes without sending anything. Repeated three times (once while blocking another user on purpose), it completed in 60 s each time. The cause was not found because that attempt did not record the page's state. If it happens again, capture what the page shows and the browser console before retrying.

## Server backups taken during the deploys

`/home/shield/backups/`: the database before migrations 0009 and 0010, and the Caddyfile before HSTS.
