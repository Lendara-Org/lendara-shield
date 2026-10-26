/**
 * The ramp timeline shown while an intent moves, derived from its status.
 * Pure, so every state of the machine (apps/api/src/intent/state.rs) is
 * covered by tests.
 */
import type { Intent, IntentStatus } from "@lendara/shield-api-types";

export type StepState = "done" | "current" | "pending" | "failed";

export interface Step {
  key: "anchor" | "received" | "deposit" | "transfer";
  label: string;
  detail: string;
  state: StepState;
  hash: string | null;
}

/** Index of the step each status is working on (4 = all done). */
const PROGRESS: Record<IntentStatus, number> = {
  created: 0,
  awaiting_anchor: 0,
  funds_received: 2,
  shielding: 2,
  transferring: 3,
  completed: 4,
  rejected: 0,
  expired: 0,
  error: 0,
};

const FAILED: ReadonlySet<IntentStatus> = new Set(["rejected", "expired", "error"]);

export function intentSteps(intent: Intent): Step[] {
  const { transactions: tx } = intent;
  const steps: Omit<Step, "state">[] = [
    {
      key: "anchor",
      label: "Pay at the anchor",
      detail: "Complete the anchor's form in local currency.",
      hash: null,
    },
    {
      key: "received",
      label: "Anchor paid Shield",
      detail: "USDC arrives at Shield's omnibus account, matched by memo.",
      hash: tx.intake_payment,
    },
    {
      key: "deposit",
      label: "Shield deposited into the pool",
      detail: "From Shield's account, ext_amount > 0. Your address is not in it.",
      hash: tx.pool_deposit,
    },
    {
      key: "transfer",
      label: "Private transfer to your keys",
      detail: "ext_amount = 0: amount and recipient are hidden.",
      hash: tx.pool_transfer,
    },
  ];

  // Before the money arrives a failure is the anchor's; after, it is the
  // pool step that was in flight (a hash is stored before it is sent).
  const failedAt = !FAILED.has(intent.status)
    ? -1
    : tx.intake_payment === null
      ? 0
      : tx.pool_transfer === null
        ? 2
        : 3;
  const current = PROGRESS[intent.status];

  return steps.map((step, i) => {
    let state: StepState;
    if (failedAt >= 0) state = i < failedAt ? "done" : i === failedAt ? "failed" : "pending";
    else if (i < current) state = "done";
    else state = i === current ? "current" : "pending";
    return { ...step, state };
  });
}
