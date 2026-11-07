import type { PoolFailureKind } from "./errors";

/**
 * What the pages need from the user's wallet and the privacy pool. The real
 * implementation (`./sdk.ts`) runs Freighter and the SPP browser SDK; tests
 * use a fake. Spending keys never leave the browser: only public keys and
 * the ASP leaf are ever sent to Shield.
 */

/** Public pool keys of a user, as `0x` hex. */
export interface RecipientKeys {
  notePublicKey: string;
  encryptionPublicKey: string;
}

export interface PrivacyKeys extends RecipientKeys {
  /** ASP membership leaf that Shield inserts in the allow-list. */
  aspLeaf: string;
}

/** On-chain transactions of one pool operation (a spend can take several). */
export interface PoolResult {
  hashes: string[];
}

export interface ShieldWallet {
  /** The connected Stellar account (`G…`). */
  readonly address: string;
  /** Derives the pool keys; the wallet asks for a signature the first time. */
  keys(): Promise<PrivacyKeys>;
  /** SEP-53 signature of `message` by the account, base64 (Freighter's `signMessage`). */
  signMessage(message: string): Promise<string>;
  /** Private balance in stroops, after syncing the pool. */
  balance(): Promise<bigint>;
  /** Direct deposit from the connected account (its address is visible). */
  deposit(amount: bigint): Promise<PoolResult>;
  /** Private transfer (`ext_amount = 0`). */
  transfer(to: RecipientKeys, amount: bigint): Promise<PoolResult>;
  /** Withdraw to the connected account. */
  withdraw(amount: bigint): Promise<PoolResult>;
}

export interface WalletConnector {
  connect(): Promise<ShieldWallet>;
}

/**
 * A pool operation the SDK reported as not done. `hashes` are the steps that
 * reached the network before it failed (a spend can take several).
 */
export class PoolOperationError extends Error {
  constructor(
    readonly status: string,
    message: string,
    readonly kind: PoolFailureKind = "other",
    readonly hashes: readonly string[] = [],
  ) {
    super(message);
    this.name = "PoolOperationError";
  }
}
