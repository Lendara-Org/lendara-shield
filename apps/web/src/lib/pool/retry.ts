/**
 * The prover's own timeout (`PROVE_TIMEOUT_MS`, 30 s). The SDK's storage
 * worker times out after 2, 5 or 15 s, so this text means the prover.
 */
const PROVER_TIMEOUT = /operation timed out after 30000 ms/;

/** What the SDK reports for a pool operation (`PoolExecuteResult`). */
export interface ExecuteResult {
  readonly status: string;
  readonly message?: string | undefined;
  readonly hashes: readonly string[];
}

/**
 * Whether an operation failed only because proving timed out before anything
 * was sent. Each step is proved before it is signed and submitted, and no
 * earlier step completed (`hashes` is empty), so nothing reached the network
 * and running it again cannot pay twice.
 */
export function provingTimedOutBeforeSending(result: ExecuteResult): boolean {
  return result.status !== "ok" && result.hashes.length === 0 && PROVER_TIMEOUT.test(result.message ?? "");
}

/**
 * Runs a pool operation and, if proving timed out before anything was sent,
 * once more: by then the prover has its circuit loaded.
 */
export async function withOneProverRetry<R extends ExecuteResult>(run: () => Promise<R>): Promise<R> {
  const first = await run();
  return provingTimedOutBeforeSending(first) ? run() : first;
}
