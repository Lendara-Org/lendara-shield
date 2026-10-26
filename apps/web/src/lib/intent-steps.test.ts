import type { IntentStatus } from "@lendara/shield-api-types";

import { HASH, completed, intent } from "@/test/fixtures";

import { intentSteps } from "./intent-steps";

const states = (status: IntentStatus, tx: Partial<ReturnType<typeof intent>["transactions"]> = {}) =>
  intentSteps(
    intent({
      status,
      transactions: { intake_payment: null, pool_deposit: null, pool_transfer: null, ...tx },
    }),
  ).map((step) => step.state);

describe("intentSteps", () => {
  it("follows the happy path", () => {
    expect(states("created")).toEqual(["current", "pending", "pending", "pending"]);
    expect(states("awaiting_anchor")).toEqual(["current", "pending", "pending", "pending"]);
    expect(states("funds_received", { intake_payment: HASH("a") })).toEqual(["done", "done", "current", "pending"]);
    expect(states("shielding", { intake_payment: HASH("a"), pool_deposit: HASH("b") })).toEqual([
      "done",
      "done",
      "current",
      "pending",
    ]);
    expect(
      states("transferring", { intake_payment: HASH("a"), pool_deposit: HASH("b"), pool_transfer: HASH("c") }),
    ).toEqual(["done", "done", "done", "current"]);
    expect(intentSteps(completed).map((s) => s.state)).toEqual(["done", "done", "done", "done"]);
  });

  it("links each on-chain step to its transaction", () => {
    expect(intentSteps(completed).map((s) => s.hash)).toEqual([null, HASH("2fc5"), HASH("d43d"), HASH("5dd9")]);
  });

  it.each(["rejected", "expired", "error"] as const)("marks the anchor step when %s before any payment", (status) => {
    expect(states(status)).toEqual(["failed", "pending", "pending", "pending"]);
  });

  it("marks the deposit when shielding fails with the money in custody", () => {
    expect(states("error", { intake_payment: HASH("a") })).toEqual(["done", "done", "failed", "pending"]);
    expect(states("error", { intake_payment: HASH("a"), pool_deposit: HASH("b") })).toEqual([
      "done",
      "done",
      "failed",
      "pending",
    ]);
  });

  it("marks the transfer when it fails", () => {
    expect(
      states("error", { intake_payment: HASH("a"), pool_deposit: HASH("b"), pool_transfer: HASH("c") }),
    ).toEqual(["done", "done", "done", "failed"]);
  });
});
