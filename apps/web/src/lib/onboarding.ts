/**
 * The text a user signs (SEP-53, with Freighter) to prove they control the
 * account they onboard. It must match the API byte for byte
 * (`apps/api/src/asp/ownership.rs`): keys in lowercase hex.
 */
import type { PrivacyKeys } from "./pool/types";

export function onboardingMessage(account: string, keys: PrivacyKeys): string {
  return [
    "Lendara Shield onboarding",
    "network: testnet",
    `account: ${account}`,
    `note_public_key: ${keys.notePublicKey.toLowerCase()}`,
    `encryption_public_key: ${keys.encryptionPublicKey.toLowerCase()}`,
    `asp_leaf: ${keys.aspLeaf.toLowerCase()}`,
  ].join("\n");
}
