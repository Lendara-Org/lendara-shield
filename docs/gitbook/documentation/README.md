---
description: KYC once, pay privately, disclose selectively.
---

# Lendara Shield

Lendara Shield gives any product on Stellar compliant private payments through one API. Wallets, anchors, payment apps and individual users get private balances, an Association Set they control and auditor-only disclosure, on top of [Stellar Private Payments](https://developers.stellar.org/docs/build/apps/privacy). Money arriving from an external source, such as any SEP-24 anchor, can reach a private balance without the user's address ever appearing on-chain.

It is the reference implementation of the **Shield Protocol**, an open draft that any wallet or anchor can adopt.

{% hint style="warning" %}
**Testnet only.** Stellar Private Payments is a developer preview with unaudited contracts. Do not use with real funds.
{% endhint %}

<table data-view="cards"><thead><tr><th></th><th></th><th data-hidden data-card-target data-type="content-ref"></th></tr></thead><tbody>
<tr><td><strong>Technical Architecture</strong></td><td>Components, Stellar integration, flows, security model and tranches.</td><td><a href="technical-architecture.md">technical-architecture.md</a></td></tr>
<tr><td><strong>Shield Protocol</strong></td><td>Level 1 (implemented) and the SEP-24 extension for native shielded deposits.</td><td><a href="shield-protocol.md">shield-protocol.md</a></td></tr>
<tr><td><strong>Threat model</strong></td><td>What is private, from whom, and the honest limits.</td><td><a href="threat-model.md">threat-model.md</a></td></tr>
<tr><td><strong>Roadmap</strong></td><td>What is done and what each tranche builds.</td><td><a href="roadmap.md">roadmap.md</a></td></tr>
</tbody></table>

## The problem

Stellar now has privacy tools in developer preview, but two things still stop products from using them:

* **A compliant pool needs an operator.** Privacy pools with an allow-list need someone to run KYC, maintain the list (the ASP), keep the auditor's view key and pay fees. Today every app would have to build that alone.
* **Funding leaks the user.** A deposit from a ramp pays the user's own address, with an amount and a time. Anyone can follow that address into a privacy pool and out of it.

## How it works

Shield runs the compliance work once, as the pool's ASP. Integrators decide which users are eligible; Shield writes the allow-list and block-list. In ramp mode, Shield receives the funding source's payment on its **omnibus account**, deposits into the pool from there, and then **transfers privately to the user's keys**. The explorer only ever sees "the source paid Shield" and "Shield used the pool". The auditor's view key shows who received what.

| | **Ramp** | **Direct** |
| --- | --- | --- |
| For | People funding from an external source, such as an anchor | People who already hold USDC |
| Money comes from | A SEP-24 anchor → Shield's omnibus account → pool | The user's own wallet → pool |
| User address on-chain | **Never** | Visible when depositing, and as the signer of their own transfers (until the relayer) |
| What Shield does | ASP, receive, deposit, transfer, audit | ASP, audit |

Both modes share the ASP, the pool client, the auditor and the intent state machine. Ramp mode talks to the funding source **only through SEPs 1, 10 and 24**, so it works with any SEP-24 anchor.

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
