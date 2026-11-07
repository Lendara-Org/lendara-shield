import { vi } from "vitest";

import type { PoolResult, ShieldWallet, WalletConnector } from "@/lib/pool/types";

import { ACCOUNT, ENC_KEY, HASH, LEAF, NOTE_KEY } from "./fixtures";

const ok = (hash: string): Promise<PoolResult> => Promise.resolve({ hashes: [HASH(hash)] });

/** A wallet whose every method is a spy with a happy default. */
export function fakeWallet(overrides: Partial<ShieldWallet> = {}): ShieldWallet {
  return {
    address: ACCOUNT,
    keys: vi.fn(() => Promise.resolve({ notePublicKey: NOTE_KEY, encryptionPublicKey: ENC_KEY, aspLeaf: LEAF })),
    signMessage: vi.fn(() => Promise.resolve("c2lnbmVkLWJ5LXRoZS1hY2NvdW50")),
    balance: vi.fn(() => Promise.resolve(93_039_215n)),
    deposit: vi.fn(() => ok("dd")),
    transfer: vi.fn(() => ok("ee")),
    withdraw: vi.fn(() => ok("ff")),
    ...overrides,
  };
}

export function connectorFor(wallet: ShieldWallet | Error): () => WalletConnector {
  return () => ({
    connect: () => (wallet instanceof Error ? Promise.reject(wallet) : Promise.resolve(wallet)),
  });
}
