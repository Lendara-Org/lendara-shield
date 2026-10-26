/**
 * A user's private receiving address: their two public pool keys in one
 * copyable string, `shield:<note key>:<encryption key>`. Shared off-chain
 * instead of registering the keys on-chain, which would expose the account.
 */
import type { RecipientKeys } from "./pool/types";

const PREFIX = "shield";
const FIELD = /^0x[0-9a-f]{64}$/;

export function encodeRecipient(keys: RecipientKeys): string {
  return `${PREFIX}:${keys.notePublicKey}:${keys.encryptionPublicKey}`;
}

/** `null` unless it is a well-formed receiving address. */
export function decodeRecipient(input: string): RecipientKeys | null {
  const [prefix, note, encryption, ...rest] = input.trim().toLowerCase().split(":");
  if (prefix !== PREFIX || rest.length > 0 || !note || !encryption) return null;
  if (!FIELD.test(note) || !FIELD.test(encryption)) return null;
  return { notePublicKey: note, encryptionPublicKey: encryption };
}
