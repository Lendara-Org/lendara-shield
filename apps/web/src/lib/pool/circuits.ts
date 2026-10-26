import { useEffect } from "react";

import { SDK_BASE_PATH } from "../config";

/**
 * The SDK's prover gives up after 30 s (`PROVE_TIMEOUT_MS`, fixed inside its
 * WASM) and on a browser's first proof that budget also pays for downloading
 * the circuit (~32 MB). Before fetching, the SDK looks in this Cache Storage
 * cache (keyed by URL) and it verifies every file against the pinned circuit
 * lock, so filling the cache ahead of time is safe.
 */
export const SDK_CIRCUIT_CACHE = "stellar-circuits-v1";

/** Where the SDK fetches circuits from (also given to `Client.new`). */
export function circuitsBaseUrl(origin: string): string {
  return `${origin}${SDK_BASE_PATH}/sdk/dist/circuits/`;
}

/** The transact circuit files of each policy (`verifiers` in the deployment). */
export function circuitUrls(base: string, policies: readonly string[]): string[] {
  return policies.flatMap((policy) => {
    const stem = `policy_tx_2_2_${policy}`;
    return [`${base}${stem}.graph.bin`, `${base}${stem}.r1cs`, `${base}${stem}_proving_key.bin`];
  });
}

/** Adds the files missing from the SDK's cache; returns how many it fetched. */
export async function preloadCircuits(urls: readonly string[], storage: CacheStorage | undefined): Promise<number> {
  if (!storage) return 0;
  const cache = await storage.open(SDK_CIRCUIT_CACHE);
  const added = await Promise.all(
    urls.map(async (url) => {
      if (await cache.match(url)) return 0;
      await cache.add(url);
      return 1;
    }),
  );
  return added.reduce<number>((sum, n) => sum + n, 0);
}

let preload: Promise<void> | null = null;

/**
 * Preloads this pool's circuits once per page. Never rejects: on failure the
 * SDK still downloads them itself, and a later call tries again.
 */
export function preloadPoolCircuits(): Promise<void> {
  preload ??= (async () => {
    const response = await fetch(`${SDK_BASE_PATH}/deployments.json`);
    if (!response.ok) throw new Error(`deployments.json: ${response.status}`);
    const deployment = (await response.json()) as { verifiers?: Record<string, unknown> };
    const policies = Object.keys(deployment.verifiers ?? {});
    await preloadCircuits(circuitUrls(circuitsBaseUrl(window.location.origin), policies), globalThis.caches);
  })().catch(() => {
    preload = null;
  });
  return preload;
}

/** Starts the preload when a page that proves opens (wallet, direct deposit). */
export function useCircuitPreload(enabled = true): void {
  useEffect(() => {
    if (enabled) void preloadPoolCircuits();
  }, [enabled]);
}
