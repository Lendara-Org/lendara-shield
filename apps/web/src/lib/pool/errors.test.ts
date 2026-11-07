import { classifyPoolFailure, isUserRejection } from "./errors";

// Texts as the SDK and the Stellar Asset Contract report them on testnet.
const DECLINED = "wallet request rejected by user: User declined access";
const NO_NOTES = "transaction planning rejected: no combination possible";
const SAC_10 =
  'transaction simulation failed: HostError: Error(Contract, #10) data:["resulting balance is not within the allowed range", 0, -10000000]';
const SAC_13 = 'HostError: Error(Contract, #13) data:["trustline entry is missing for account", G…]';

describe("classifyPoolFailure", () => {
  it("recognises a declined signature in any operation", () => {
    for (const op of ["deposit", "transfer", "withdraw"] as const) {
      expect(classifyPoolFailure(op, "failed", DECLINED)).toBe("rejected");
    }
  });

  it("reads the planner's failure as a short private balance, but not on a deposit", () => {
    expect(classifyPoolFailure("transfer", "failed", NO_NOTES)).toBe("insufficientPrivate");
    expect(classifyPoolFailure("withdraw", "failed", NO_NOTES)).toBe("insufficientPrivate");
    expect(classifyPoolFailure("deposit", "failed", NO_NOTES)).toBe("other");
  });

  it("reads the token's balance error as a short public balance only on a deposit", () => {
    expect(classifyPoolFailure("deposit", "failed", SAC_10)).toBe("insufficientPublic");
    expect(classifyPoolFailure("withdraw", "failed", SAC_10)).toBe("other");
  });

  it("recognises a missing trustline and the ASP not being ready", () => {
    expect(classifyPoolFailure("deposit", "failed", SAC_13)).toBe("trustlineMissing");
    expect(classifyPoolFailure("transfer", "aspNotReady", "")).toBe("aspNotReady");
  });

  it("leaves anything else as other", () => {
    expect(classifyPoolFailure("deposit", "failed", "operation timed out after 30000 ms")).toBe("other");
    expect(classifyPoolFailure("transfer", "failed", "")).toBe("other");
  });
});

describe("isUserRejection", () => {
  it("is true for Freighter's rejection code or text, false otherwise", () => {
    expect(isUserRejection(Object.assign(new Error("whatever"), { code: -4 }))).toBe(true);
    expect(isUserRejection(new Error("User declined access"))).toBe(true);
    expect(isUserRejection(new Error("RPC unavailable"))).toBe(false);
    expect(isUserRejection("User declined")).toBe(false);
  });
});
