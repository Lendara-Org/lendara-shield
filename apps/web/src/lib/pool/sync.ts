/**
 * The SDK can fail while decoding a transient RPC response during event
 * catch-up. Retrying this read resumes its persisted cursor; it never signs
 * or submits a payment. Keep this separate from operation/prover retries.
 */
export function isTransientSyncFailure(error: unknown): boolean {
  const message = error instanceof Error ? error.message : typeof error === "string" ? error : "";
  return /indexer(?: catch-up)?: network error:/i.test(message);
}

/** Only for reads: shares concurrent callers and retries transient catch-up twice. */
export function poolRead<T>(run: () => Promise<T>): () => Promise<T> {
  let pending: Promise<T> | null = null;

  async function attempt(): Promise<T> {
    for (let retry = 0; ; retry += 1) {
      try {
        return await run();
      } catch (error) {
        if (retry >= 2 || !isTransientSyncFailure(error)) throw error;
        await new Promise<void>((resolve) => setTimeout(resolve, 250 * 2 ** retry));
      }
    }
  }

  return () => {
    pending ??= attempt().finally(() => {
      pending = null;
    });
    return pending;
  };
}
