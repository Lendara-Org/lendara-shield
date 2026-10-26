# Demo video script (2–3 minutes)

The story of **María**, a freelance designer in Costa Rica who gets paid in colones and saves in dollars. Record at 1280×800 with Freighter on Testnet, a fresh account, and the API and web running (`make dev`). Prepare one ramp deposit **before** recording, so its explorer links are ready if the live one takes longer.

| Time | On screen | Voice-over |
|---|---|---|
| 0:00–0:15 | A stellar.expert page: a normal SEP-24 deposit to someone's `G…` address, then that address's history | "This is how a ramp deposit looks on a public blockchain today. Anyone can see who got the money, how much, and follow it from there. For María, that's her income, public forever." |
| 0:15–0:30 | Lendara Shield landing (`/`) | "Lendara Shield fixes privacy at the fiat edge. KYC once, pay privately, disclose selectively. It works with any SEP-24 anchor and Stellar Private Payments." |
| 0:30–0:55 | `/onboard`: connect Freighter, choose **Approved**, *Derive keys and register*, the ASP transaction link appears | "María connects her wallet. Her private keys are derived right here in the browser and never leave it. Shield only gets her public keys. After the KYC, Shield, acting as the pool's gatekeeper, adds her to the allow-list on-chain." |
| 0:55–1:30 | `/deposit` → *With a ramp* → CRC 5000 → anchor widget → test card 4242… → back to Shield: the timeline fills step by step | "She deposits 5,000 colones through the anchor. The anchor doesn't pay María. It pays Shield's account, tagged with a memo. Shield deposits into the privacy pool and then transfers to María's keys privately. No human in the loop: about a minute." |
| 1:30–1:55 | Open the three explorer links side by side; highlight that María's address is not there; `ext_amount > 0` on the deposit, nothing on the transfer | "On the explorer: the anchor paid Shield, Shield used the pool. María's address appears nowhere, and the last transfer shows no amount and no recipient." |
| 1:55–2:10 | `/wallet`: her private balance in USDC | "Yet the money is hers. Only she can see and spend this balance." |
| 2:10–2:35 | `/audit`: paste the token; the same transfer side by side, explorer vs auditor | "And it's still compliant. The auditor's view key, which never leaves Shield's server, shows exactly who received what. Private to the public, accountable to the regulator." |
| 2:35–2:50 | README: two modes, the Shield Protocol draft | "Shield works as middleware for any SEP-24 anchor today. Our Shield Protocol draft lets anchors deposit into the pool natively." |
| 2:50–3:00 | Logo and repo URL | "Lendara Shield. KYC once, pay privately, disclose selectively." |

**Honest notes for the Q&A** (from the [threat model](threat-model.md)):
- The anchor still knows María and the amount.
- On testnet, the anchor's payment and Shield's deposit can be linked by amount and time; batching and rounding are the next step.
- In direct mode the user's own address signs.
