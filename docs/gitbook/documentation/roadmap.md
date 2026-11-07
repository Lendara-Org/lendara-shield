---
description: What is done, and what each SCF tranche builds.
---

# Roadmap

Both modes already run end to end on Stellar testnet. The three tranches take Shield from a working testnet build to mainnet.

## Done

**HackMeridian 2026 (October 25–26):** SPP spike, OpenAPI contract, testnet pool, onboarding and ASP, intents and anchor client, pool CLI, watcher and shielding worker, web app in English and Spanish, audit panel and docs. Ramp mode completes on testnet with the user's address appearing nowhere on-chain.

**Security fixes after the hackathon:**

* Anchor-confirmed funding and signed onboarding (SEP-53).
* Signed, single-use intent requests; the interactive URL only in the creation response.
* Blocked users stopped mid-intent; a bounded wait for unconfirmed anchor payments.
* Separate ramp and shielding loops; per-IP rate limit; ASP routes with their own timeout.
* A Content-Security-Policy and startup checks on keys and URLs.

## SCF Build Award tranches

| Tranche | Builds |
| --- | --- |
| **1 — Developer-ready testnet** | ASP leaf bound to the registered key (closes known limit 7) · fee-bump relayer (direct-mode users no longer visible as signers) · project API keys, scoped permissions and sessions, including the integrator's eligibility decision through its key · TypeScript SDK generated from `openapi.yaml` |
| **2 — Production readiness** | Shield Protocol level 2: complete specification that any SEP-24 anchor can adopt, with Lendara's anchor as the first reference implementation (direct deposits into the pool, no custody step) · operator, omnibus and view keys in a KMS/HSM with rotation · per-note selective disclosure for users and a role-based auditor portal with access logs · resolution of blocked users' funds (resume, return to the anchor or hold) with an audit log · integration with a second funding source · threat model (STRIDE-style) and monitoring plan for the production deployment |
| **3 — Mainnet** | Mainnet deployment with archival RPC, monitoring and alerts · anonymity-set hardening (batching, rounding, delays) · first third-party wallet integration |

## After this award: one compliance layer for both privacy primitives

Stellar has two complementary privacy tools. **Stellar Private Payments** hides who pays whom; **Confidential Tokens** hides amounts and balances while addresses stay visible. The SCF tranches build the Shield Protocol on SPP first. Next, the same compliance layer (the integrator's eligibility decision, the ASP role and auditor-only disclosure) extends to Confidential Tokens, so wallets and anchors can offer private counterparties or private amounts through the same API. This is not part of the requested budget.

How each item fits the system is in [Technical Architecture](technical-architecture.md). The limits each one addresses are in the [Threat model](threat-model.md).
