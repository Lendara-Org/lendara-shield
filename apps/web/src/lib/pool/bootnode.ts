/**
 * Proving needs the pool's full event history and the public RPC keeps
 * about 7 days of it. An older pool needs SPP's bootnode for the rest: the
 * SDK reads the old events from it and hands off to the RPC for the recent
 * ones.
 */

/**
 * The SDK's bootnode option: the configured `url` if any. Without one,
 * `isRequired` asks the SDK whether the RPC still holds the whole history,
 * and a pool older than that fails here instead of mid-operation.
 */
export async function bootnodeOption(
  url: string | undefined,
  isRequired: () => Promise<boolean>,
): Promise<{ bootnodeUrl?: string }> {
  if (url) return { bootnodeUrl: url };
  if (await isRequired()) {
    throw new Error("The pool is older than the RPC's history window and no bootnode is configured (NEXT_PUBLIC_BOOTNODE_URL).");
  }
  return {};
}
