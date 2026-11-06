---
description: Run Lendara Shield on your machine against Stellar testnet.
---

# Run locally

Requirements: Node 24, pnpm 12, Rust stable, Docker, the [`stellar` CLI](https://developers.stellar.org/docs/tools/cli) and [Freighter](https://www.freighter.app/) set to **Testnet**.

```bash
cp .env.example .env
make install
make db-up
make setup-testnet        # funds Shield's accounts, deploys the pool and initialises the intake
                          # (idempotent); copy the variables from
                          # ~/.local/share/lendara-shield/testnet/testnet.env into .env
make build-pool-cli
make dev                  # API on :8080, web on http://localhost:3000
```

Then set `ADMIN_TOKEN` and `AUDITOR_TOKEN` in `.env` (`openssl rand -hex 32`). Ramp mode uses the public Lendara sandbox anchor by default (`RAMP_ANCHOR_HOME_DOMAIN`); leave it empty to run with direct mode only.

Checks: `make lint`, `make test` (Postgres needed), `make test-web-e2e`, and `make test-testnet` for read-only checks against testnet.

## 5-minute demo

{% stepper %}
{% step %}
### Onboard

Open `http://localhost:3000/en/onboard` (or `/es/…` in Spanish) and connect Freighter (Testnet). Choose the eligibility scenario **Approved** and click *Derive keys and register*. Freighter signs twice: once to derive your pool keys in the browser, once to prove the account is yours. Shield then adds your leaf to the allow-list.
{% endstep %}

{% step %}
### Deposit with a ramp

On `/en/deposit`, choose *With a ramp*, enter `CRC 5000` and continue. In the anchor's window, pay with the test card **4242 4242 4242 4242** (any future date and any CVC). Back in Shield, the timeline fills on its own: anchor paid Shield → Shield deposited into the pool → private transfer to your keys.
{% endstep %}

{% step %}
### Check the explorer

Open the three links: your address is in none of them. The deposit shows `ext_amount > 0` from Shield's account. The transfer shows nothing but Shield's account.
{% endstep %}

{% step %}
### See your private balance

On `/en/wallet`, the USDC is there. Only you, and the auditor, can see it.
{% endstep %}

{% step %}
### Audit

On `/en/audit`, paste `AUDITOR_TOKEN`. The explorer sees *"signed by Shield, amount hidden, recipient not visible"*; the auditor sees *"9.30 USDC to you"*.
{% endstep %}
{% endstepper %}

To try direct mode, choose *From my wallet* on `/en/deposit`. Your account needs testnet USDC from the sandbox and its trustline. Every eligibility and funding outcome is listed in [Scenarios](scenarios.md).
