import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { IntentStatus } from "../src/index.ts";
import { FINAL_INTENT_STATUSES, isFinalIntentStatus } from "../src/index.ts";

const IN_PROGRESS: IntentStatus[] = [
  "created",
  "awaiting_anchor",
  "funds_received",
  "shielding",
  "transferring",
];

describe("isFinalIntentStatus", () => {
  it("is true for every final status", () => {
    for (const status of FINAL_INTENT_STATUSES) {
      assert.equal(isFinalIntentStatus(status), true, status);
    }
  });

  it("is false while the intent is in progress", () => {
    for (const status of IN_PROGRESS) {
      assert.equal(isFinalIntentStatus(status), false, status);
    }
  });
});
