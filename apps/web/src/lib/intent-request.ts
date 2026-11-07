/**
 * Signed `POST /v1/intents` requests. Only the account's owner can start a
 * deposit: the user signs (SEP-53, with Freighter) the amount and a
 * single-use request id. The text must match the API byte for byte
 * (`intent_message` in `apps/api/src/asp/ownership.rs`).
 */
import type { CreateIntentRequest } from "@lendara/shield-api-types";

import type { ShieldWallet } from "./pool/types";

export function intentMessage(
  account: string,
  fiatCurrency: string,
  amount: string,
  requestId: string,
  issuedAt: number,
): string {
  return [
    "Lendara Shield intent",
    "network: testnet",
    `account: ${account}`,
    `fiat_currency: ${fiatCurrency}`,
    `amount: ${amount}`,
    `request_id: ${requestId.toLowerCase()}`,
    `issued_at: ${issuedAt}`,
  ].join("\n");
}

/** A ramp intent request for `wallet`'s account, signed by it. */
export async function signedIntentRequest(
  wallet: Pick<ShieldWallet, "address" | "signMessage">,
  fiatCurrency: string,
  amount: string,
  now: () => number = Date.now,
  newId: () => string = () => crypto.randomUUID(),
): Promise<CreateIntentRequest> {
  const requestId = newId().toLowerCase();
  const issuedAt = Math.floor(now() / 1000);
  const signature = await wallet.signMessage(intentMessage(wallet.address, fiatCurrency, amount, requestId, issuedAt));
  return {
    mode: "ramp",
    stellar_account: wallet.address,
    fiat_currency: fiatCurrency,
    amount,
    request_id: requestId,
    issued_at: issuedAt,
    signature,
  };
}
