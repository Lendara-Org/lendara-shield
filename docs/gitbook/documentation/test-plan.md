---
description: How Lendara Shield is tested, from unit tests to real runs on testnet.
---

# Test plan

Every change ships with tests for what it adds, including its error paths. Changes that touch the network (pool, ASP, payments) also ship with real testnet transactions.

## Test levels

| Level | What it covers | How |
| --- | --- | --- |
| Unit | Pure domain: intent transitions, ASP rules, validated types, amounts, memo matching, SEP-10 challenge validation, SEP-53 signatures | Rust `#[test]` next to the code |
| Database | Stores, constraints and transactions | `#[sqlx::test]`, one isolated PostgreSQL database per test |
| HTTP | Every endpoint: success, 400, 401, 404, 409 and 503 | The real router with fake ports |
| Adapters | Horizon, Soroban RPC, the anchor (SEP-1/10/24) and `pool-cli` | `wiremock` with recorded public testnet responses; a fake CLI |
| Worker | Idempotency under failure: ambiguous submits, dropped and refused transactions, RPC lag and a chaotic mix | A fake chain that enforces Stellar's sequence rule; every test asserts exactly one deposit and one transfer |
| Web | Logic, components and API calls | Vitest, Testing Library and MSW; a Playwright smoke test in a real browser |
| Testnet | Accounts, trustline, asset contract, pool configuration, SPP verification, bootnode, sandbox anchor | `make test-testnet` and ignored Rust tests run against testnet |

Coverage targets: at least 90% in the domain, intent and ASP modules, and at least 75% overall.

## Continuous integration

Every pull request runs formatting, linting, type checks, the full test suites, license and advisory checks on Rust dependencies (cargo deny), the build, the Playwright smoke test, a check that the OpenAPI contract and the generated types match, a secret scan (gitleaks) and a workflow and script lint (actionlint, shellcheck).

## Runs on testnet

Beyond the automated suites, the team runs the product end to end on testnet and publishes the results with transaction hashes:

| Date | Report | Result |
| --- | --- | --- |
| 2026-10-28 | [Testnet verification](https://github.com/Lendara-Org/lendara-shield/blob/develop/docs/testing/2026-10-28-testnet-verification.md) | End-to-end circuit, configuration, errors and recovery; 2 findings |
| 2026-11-06 | [Testnet verification, section 6](testnet-verification.md) | Every flow on the redeployed pool, with hashes |
| 2026-11-06 | [RPC latency](https://github.com/Lendara-Org/lendara-shield/blob/develop/docs/testing/2026-11-06-rpc-latency.md) | Slow shielding traced to the public RPC's latency |
| 2026-11-07 | [Exhaustive audit](https://github.com/Lendara-Org/lendara-shield/blob/develop/docs/testing/2026-11-07-exhaustive-audit.md) | Full product and black-box audit (Rust 299, web 143 and Playwright 13 tests passing; coverage 90.6%); 7 findings, all fixed with regression tests |

## For each SCF tranche

Each deliverable is complete only when it has tests for its success and error paths, the CI is green, and, if it touches the network, real testnet (or mainnet) transactions are published. Tranche 2 extends the threat model into a STRIDE-style analysis with a monitoring plan; Tranche 3 repeats the end-to-end run on mainnet.
