import { provingTimedOutBeforeSending, withOneProverRetry, type ExecuteResult } from "./retry";

const timeout: ExecuteResult = { status: "failed", message: "operation timed out after 30000 ms", hashes: [] };
const ok: ExecuteResult = { status: "ok", hashes: ["ab"] };

describe("provingTimedOutBeforeSending", () => {
  it("is true only for the prover's timeout with nothing sent", () => {
    expect(provingTimedOutBeforeSending(timeout)).toBe(true);
    // An earlier step of the plan landed: running it again could pay twice.
    expect(provingTimedOutBeforeSending({ ...timeout, hashes: ["ab"] })).toBe(false);
    // The storage worker's timeouts (5 s) are not the prover's.
    expect(provingTimedOutBeforeSending({ ...timeout, message: "operation timed out after 5000 ms" })).toBe(false);
    expect(provingTimedOutBeforeSending({ status: "failed", message: "tx_bad_seq", hashes: [] })).toBe(false);
    expect(provingTimedOutBeforeSending({ status: "aspNotReady", hashes: [] })).toBe(false);
    expect(provingTimedOutBeforeSending(ok)).toBe(false);
  });
});

describe("withOneProverRetry", () => {
  it("runs once more after a proving timeout", async () => {
    const run = vi.fn<() => Promise<ExecuteResult>>().mockResolvedValueOnce(timeout).mockResolvedValueOnce(ok);
    await expect(withOneProverRetry(run)).resolves.toBe(ok);
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("retries at most once", async () => {
    const run = vi.fn<() => Promise<ExecuteResult>>().mockResolvedValue(timeout);
    await expect(withOneProverRetry(run)).resolves.toBe(timeout);
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("does not retry other outcomes", async () => {
    for (const result of [ok, { ...timeout, hashes: ["ab"] }, { status: "failed", message: "tx_bad_seq", hashes: [] }]) {
      const run = vi.fn<() => Promise<ExecuteResult>>().mockResolvedValue(result);
      await expect(withOneProverRetry(run)).resolves.toBe(result);
      expect(run).toHaveBeenCalledTimes(1);
    }
  });

  it("does not retry a rejected operation", async () => {
    const run = vi.fn<() => Promise<ExecuteResult>>().mockRejectedValue(new Error("Freighter closed"));
    await expect(withOneProverRetry(run)).rejects.toThrow("Freighter closed");
    expect(run).toHaveBeenCalledTimes(1);
  });
});
