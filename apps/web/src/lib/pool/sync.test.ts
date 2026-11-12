import { isTransientSyncFailure, poolRead } from "./sync";

const malformedEvents = new Error(
  'indexer catch-up: network error: error decoding response body: missing field `events` at line 1 column 86',
);

afterEach(() => vi.useRealTimers());

describe("poolRead", () => {
  it("recovers automatically from a malformed RPC event response", async () => {
    vi.useFakeTimers();
    const run = vi.fn<() => Promise<bigint>>().mockRejectedValueOnce(malformedEvents).mockResolvedValue(14_607_843n);
    const sync = poolRead(run);
    const result = sync();

    await vi.runAllTimersAsync();
    await expect(result).resolves.toBe(14_607_843n);
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("shares one catch-up across concurrent callers, including its retries", async () => {
    vi.useFakeTimers();
    const run = vi.fn<() => Promise<void>>().mockRejectedValueOnce(malformedEvents).mockResolvedValue(undefined);
    const sync = poolRead(run);
    const first = sync();
    const second = sync();

    expect(second).toBe(first);
    await vi.runAllTimersAsync();
    await Promise.all([first, second]);
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("stops after three failed reads and preserves the error", async () => {
    vi.useFakeTimers();
    const run = vi.fn<() => Promise<void>>().mockRejectedValue(malformedEvents);
    const sync = poolRead(run);
    const result = expect(sync()).rejects.toBe(malformedEvents);

    await vi.runAllTimersAsync();
    await result;
    expect(run).toHaveBeenCalledTimes(3);
  });

  it("does not retry storage, proof, authorization or history failures", async () => {
    for (const message of ["storage worker failed", "proof failed", "user rejected", "RPC history is missing"]) {
      const error = new Error(message);
      const run = vi.fn<() => Promise<void>>().mockRejectedValue(error);
      await expect(poolRead(run)()).rejects.toBe(error);
      expect(run).toHaveBeenCalledTimes(1);
    }
  });

  it("starts a fresh read after either success or failure", async () => {
    const error = new Error("storage worker failed");
    const run = vi.fn<() => Promise<void>>()
      .mockRejectedValueOnce(error)
      .mockResolvedValue(undefined);
    const sync = poolRead(run);

    await expect(sync()).rejects.toBe(error);
    await sync();
    await sync();
    expect(run).toHaveBeenCalledTimes(3);
  });
});

describe("isTransientSyncFailure", () => {
  it("recognizes the SDK's event-network context without retrying unrelated errors", () => {
    expect(isTransientSyncFailure(malformedEvents)).toBe(true);
    expect(isTransientSyncFailure(malformedEvents.message)).toBe(true);
    expect(isTransientSyncFailure(new Error('indexer: network error: missing field `events`'))).toBe(true);
    expect(isTransientSyncFailure(new Error('missing field `events`'))).toBe(false);
    expect(isTransientSyncFailure(null)).toBe(false);
    expect(isTransientSyncFailure({ message: malformedEvents.message })).toBe(false);
  });
});
