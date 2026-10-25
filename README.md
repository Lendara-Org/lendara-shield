# Lendara Shield

> **KYC once, pay privately, disclose selectively.**

Lendara Shield is the reference implementation of the **Shield Protocol**: a compliant privacy layer for Stellar that connects fiat on-ramps (SEP-24 anchors) to a [Stellar Private Payments](https://developers.stellar.org/docs/build/apps/privacy) pool, so the user's address never appears on-chain when money comes in.

> ⚠️ **Testnet only.** Stellar Private Payments is a developer preview with unaudited contracts. Do not use with real funds.

## The problem

_TODO (H9)._

## Two modes, one core

| | `ramp` | `direct` |
|---|---|---|
| Money comes from | Local currency through a SEP-24 anchor | USDC already in the user's wallet |
| User address on-chain at entry | No (Shield's omnibus account deposits) | Yes (documented trade-off) |

## Architecture

_TODO (H9)._

## Run locally

Requirements: Node 24, pnpm 12, Rust stable, Docker.

```bash
cp .env.example .env
make install
make db-up
make help
```

## 5-minute demo

_TODO (H9)._

## Team

Anouk, Karu and Elisa — built for HackMeridian 2026.

## License

[Apache-2.0](LICENSE)
