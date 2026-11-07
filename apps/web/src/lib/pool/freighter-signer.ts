/**
 * Freighter as the SDK's `WalletSigner`. Written here (instead of the SDK's
 * `stellar-private-payments/freighter`) because the SDK is loaded at runtime
 * from `/spp`, outside the bundle, while Freighter's API is bundled.
 */
import {
  getNetworkDetails,
  isConnected,
  requestAccess,
  signAuthEntry,
  signMessage,
  signTransaction,
} from "@stellar/freighter-api";
import type { SignOptions, WalletSigner } from "stellar-private-payments";

/** SEP-43 code for "the user declined". */
const USER_REJECTED = -4;

export class WalletError extends Error {
  constructor(
    message: string,
    readonly code?: number,
  ) {
    super(message);
    this.name = "WalletError";
  }

  get rejected(): boolean {
    return this.code === USER_REJECTED;
  }
}

type FreighterError = { message?: string; code?: number } | undefined;

function check(error: FreighterError, fallback: string): void {
  if (error) throw new WalletError(error.message ?? fallback, error.code);
}

/** Asks Freighter for access and returns the account, on the expected network. */
export async function connectFreighter(networkPassphrase: string): Promise<string> {
  const connected = await isConnected();
  if (!connected.isConnected) {
    throw new WalletError("Freighter is not installed. Get it at freighter.app and reload.");
  }
  const access = await requestAccess();
  check(access.error, "Freighter did not grant access");
  const network = await getNetworkDetails();
  check(network.error, "Could not read Freighter's network");
  if (network.networkPassphrase !== networkPassphrase) {
    throw new WalletError("Switch Freighter to Testnet and try again.");
  }
  return access.address;
}

export class FreighterSigner implements WalletSigner {
  async signMessage(message: string, opts: SignOptions = {}) {
    const result = await signMessage(message, opts);
    check(result.error, "Message signing failed");
    if (!result.signedMessage) throw new WalletError("Freighter returned no signature");
    return { signedMessage: String(result.signedMessage), ...signer(result.signerAddress) };
  }

  async signTransaction(xdr: string, opts: SignOptions = {}) {
    const result = await signTransaction(xdr, opts);
    check(result.error, "Transaction signing failed");
    return { signedTxXdr: result.signedTxXdr, ...signer(result.signerAddress) };
  }

  async signAuthEntry(xdr: string, opts: SignOptions = {}) {
    const result = await signAuthEntry(xdr, opts);
    check(result.error, "Authorization signing failed");
    if (!result.signedAuthEntry) throw new WalletError("Freighter returned no signed authorization");
    return { signedAuthEntry: String(result.signedAuthEntry), ...signer(result.signerAddress) };
  }
}

const signer = (address: string | undefined) => (address ? { signerAddress: address } : {});
