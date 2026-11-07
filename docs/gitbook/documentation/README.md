---
description: An open protocol for compliant private payments on Stellar.
---

# Lendara Shield

Lendara Shield gives any product on Stellar compliant private payments through one API. Wallets and anchors integrate it to give their users private balances, with an Association Set the integrator controls and auditor-only disclosure, on top of [Stellar Private Payments](https://developers.stellar.org/docs/build/apps/privacy). Money arriving from an external funding source can reach a private balance without the user's address ever appearing on-chain.

It is the reference implementation of the **Shield Protocol**, an open draft that any integrator can adopt.

{% hint style="warning" %}
**Testnet only.** Stellar Private Payments is a developer preview with unaudited contracts. Do not use with real funds.
{% endhint %}

<table data-view="cards"><thead><tr><th></th><th></th><th data-hidden data-card-target data-type="content-ref"></th></tr></thead><tbody>
<tr><td><strong>Technical Architecture</strong></td><td>Components, Stellar integration, flows, security model and tranches.</td><td><a href="technical-architecture.md">technical-architecture.md</a></td></tr>
<tr><td><strong>Shield Protocol</strong></td><td>Level 1 (implemented) and level 2: funding sources depositing straight into the pool.</td><td><a href="shield-protocol.md">shield-protocol.md</a></td></tr>
<tr><td><strong>Threat model</strong></td><td>What is private, from whom, and the honest limits.</td><td><a href="threat-model.md">threat-model.md</a></td></tr>
<tr><td><strong>Roadmap</strong></td><td>What is done and what each tranche builds.</td><td><a href="roadmap.md">roadmap.md</a></td></tr>
</tbody></table>

## The problem

Stellar now has privacy tools in developer preview, but two things still stop products from using them:

* **A compliant pool needs an operator.** Privacy pools with an allow-list need someone to maintain the list (the ASP), keep the auditor's view key and pay fees. Today every app would have to build that alone.
* **Funding leaks the user.** A deposit from an external source pays the user's own address, with an amount and a time. Anyone can follow that address into a privacy pool and out of it.

## How it works

Shield operates the pool's ASP. Each integrator decides which of its users are eligible under its own policy; Shield only writes that decision to the allow-list and block-list. In ramp mode, Shield receives the funding source's payment on its **omnibus account**, deposits into the pool from there, and then **transfers privately to the user's keys**. The explorer only ever sees "the source paid Shield" and "Shield used the pool". The auditor's view key shows who received what.

| | **Ramp** | **Direct** |
| --- | --- | --- |
| For | Users funded by an external source | Users who already hold USDC |
| Money comes from | Funding source → Shield's omnibus account → pool | The user's own wallet → pool |
| User address on-chain | **Never** | Visible when depositing, and as the signer of their own transfers (until the relayer) |
| What Shield does | ASP, receive, deposit, transfer, audit | ASP, audit |

Both modes share the ASP, the pool client, the auditor and the intent state machine. Ramp mode talks to the funding source **only through standard SEPs**. The first supported source type is a SEP-24 anchor (SEPs 1, 10 and 24), with no changes on its side.

## Who integrates Shield

Shield's customers are **wallets** and **anchors**. Individuals are the end users: they reach Shield through a wallet or an anchor, which decides whether they are eligible. A compliant pool needs an accountable party behind that decision, so a person cannot join on their own.

| | **Wallets** | **Anchors** |
| --- | --- | --- |
| What their users get | Private balances: deposit, private transfer and withdraw, with proofs generated on the user's device | Deposits that arrive as a private balance, with the user's address never on-chain |
| How they integrate | Shield's API (OpenAPI) with a project API key, and the TypeScript SDK (Tranche 1) | **Level 1:** no changes; Shield acts as a SEP-24 client in front of the anchor. **Level 2:** the anchor adds the SEP-24 extension and deposits straight into the pool (Tranche 2) |
| What they decide | Which of their users are eligible, under their own policy (which may include KYC) | The same, plus their usual checks as a funding source |
| What Shield does | Writes the allow-list and block-list (ASP), protects the auditor's view key, pays fees through a relayer (Tranche 1) | Same ASP and auditor role; in level 1, also the omnibus step |

## Live on testnet

| | |
| --- | --- |
| Web app (English / Spanish) | [lendara-shield.duckdns.org/en](https://lendara-shield.duckdns.org/en) · [/es](https://lendara-shield.duckdns.org/es) |
| Developer docs | [lendara-shield.duckdns.org/en/docs](https://lendara-shield.duckdns.org/en/docs) |
| API reference (from `openapi.yaml`) | [api-reference.html](https://lendara-shield.duckdns.org/api-reference.html) |
| Repository | [github.com/Lendara-Org/lendara-shield](https://github.com/Lendara-Org/lendara-shield) |

Real testnet transactions for both modes are listed in [Technical Architecture, Section 9](technical-architecture.md#9.-what-exists-today-testnet-verifiable).

## Team

[Lendara](https://github.com/Lendara-Org):

* **Anouk Rímola** — [LinkedIn](https://www.linkedin.com/in/anouk-r%C3%ADmola-b9915a223/)
* **María Elisa Araya** — [LinkedIn](https://www.linkedin.com/in/arayamariaelisa/)
* **Karen Giannetto** — [LinkedIn](https://www.linkedin.com/in/karen-giannetto/)

Licensed under [Apache-2.0](https://github.com/Lendara-Org/lendara-shield/blob/main/LICENSE).
