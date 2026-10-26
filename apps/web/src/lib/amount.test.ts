import { formatAmount, parseAmount } from "./amount";

describe("parseAmount", () => {
  it.each([
    ["1", 10_000_000n],
    ["9.3039215", 93_039_215n],
    ["0.0000001", 1n],
    [" 1.5 ", 15_000_000n],
    ["123456789012.1234567", 1_234_567_890_121_234_567n],
  ])("parses %j exactly", (input, stroops) => {
    expect(parseAmount(input)).toBe(stroops);
  });

  it.each(["", "0", "0.0", "-1", "1.", ".5", "01", "1.12345678", "1e3", "1,5", "abc"])("rejects %j", (input) => {
    expect(parseAmount(input)).toBeNull();
  });
});

describe("formatAmount", () => {
  it.each([
    [93_039_215n, "9.3039215"],
    [10_000_000n, "1"],
    [15_000_000n, "1.5"],
    [1n, "0.0000001"],
    [0n, "0"],
    [-15_000_000n, "-1.5"],
  ])("formats %s as %j", (stroops, text) => {
    expect(formatAmount(stroops)).toBe(text);
  });

  it("round-trips", () => {
    for (const text of ["1", "9.3039215", "0.0000001", "42.42"]) {
      expect(formatAmount(parseAmount(text) ?? -1n)).toBe(text);
    }
  });
});
