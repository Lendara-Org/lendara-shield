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
| **1 — Developer-ready testnet** | Fee-bump relayer (direct-mode users no longer visible as signers) · project API keys, scoped permissions and sessions, including the integrator's eligibility decision through its key · TypeScript SDK generated from `openapi.yaml` |
| **2 — Production readiness** | Operator, omnibus and view keys in a KMS/HSM with rotation · per-note selective disclosure for users and a role-based auditor portal with access logs · integration with a second funding source |
| **3 — Mainnet** | Mainnet deployment with archival RPC, monitoring and alerts · anonymity-set hardening (batching, rounding, delays) · first third-party wallet integration |

How each item fits the system is in [Technical Architecture](technical-architecture.md). The limits each one addresses are in the [Threat model](threat-model.md).
