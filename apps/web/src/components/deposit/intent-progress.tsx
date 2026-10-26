"use client";

import type { Intent } from "@lendara/shield-api-types";
import { isFinalIntentStatus } from "@lendara/shield-api-types";
import { useState } from "react";

import { Card, Notice, TxLink } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { INTENT_POLL_MS } from "@/lib/config";
import { usePoll } from "@/lib/hooks/use-poll";
import { type StepState, intentSteps } from "@/lib/intent-steps";

const MARK: Record<StepState, string> = {
  done: "bg-ink text-paper",
  current: "border-2 border-crimson text-crimson",
  pending: "border border-line text-gray",
  failed: "bg-error text-paper",
};

/** Live timeline of a ramp intent, polled until it reaches a final state. */
export function IntentProgress({ initial }: { initial: Intent }) {
  const [intent, setIntent] = useState(initial);
  const final = isFinalIntentStatus(intent.status);

  usePoll(!final, INTENT_POLL_MS, async () => {
    setIntent(await api.intent(intent.id));
  });

  return (
    <Card title="Your private deposit">
      <ol className="space-y-4" aria-live="polite">
        {intentSteps(intent).map((step, i) => (
          <li key={step.key} className="flex gap-3" data-state={step.state}>
            <span aria-hidden className={`flex size-6 shrink-0 items-center justify-center rounded-full text-xs ${MARK[step.state]}`}>
              {step.state === "done" ? "✓" : step.state === "failed" ? "!" : i + 1}
            </span>
            <div className="space-y-0.5">
              <p className="text-sm font-medium">
                {step.label}
                <span className="sr-only">: {step.state}</span>
              </p>
              <p className="text-xs text-gray">{step.detail}</p>
              {step.hash && <TxLink hash={step.hash} />}
            </div>
          </li>
        ))}
      </ol>
      <div className="mt-5">
        {intent.status === "completed" && (
          <Notice tone="ok">
            {intent.amount_asset} {intent.asset_code} are now private in your pool balance.
          </Notice>
        )}
        {final && intent.status !== "completed" && (
          <Notice tone="error">
            Deposit {intent.status}
            {intent.message ? `: ${intent.message}` : "."}
          </Notice>
        )}
      </div>
    </Card>
  );
}
