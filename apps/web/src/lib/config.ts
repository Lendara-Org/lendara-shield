/** Public runtime configuration (baked in at build time by Next). */

/** Base URL of the Shield API, without a trailing slash. */
export const API_BASE_URL = (process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8080").replace(/\/$/, "");

/** Stellar RPC the browser SDK syncs and proves against. */
export const SOROBAN_RPC_URL = process.env.NEXT_PUBLIC_SOROBAN_RPC_URL ?? "https://soroban-testnet.stellar.org";

/** Where `scripts/stage-web-sdk.sh` serves the pool SDK and the deployment. */
export const SDK_BASE_PATH = "/spp";

/** How often a pending intent is polled. */
export const INTENT_POLL_MS = 2500;
