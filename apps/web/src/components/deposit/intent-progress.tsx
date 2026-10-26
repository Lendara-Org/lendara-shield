"use client";

import type { Intent } from "@lendara/shield-api-types";
import { isFinalIntentStatus } from "@lendara/shield-api-types";
import { useState } from "react";

import { useI18n } from "@/components/i18n";
import { Card, Notice, TxLink } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { INTENT_POLL_MS } from "@/lib/config";
import { usePoll } from "@/lib/hooks/use-poll";
import { type StepState, intentSteps } from "@/lib/intent-steps";

const MARK: Record<StepState, string> = {
  done: "bg-success-subtle text-success",
  current: "border-2 border-action-primary text-action-primary",
  pending: "border border-line text-gray",
  failed: "bg-error text-paper",
};

/** Live timeline of a ramp intent, polled until it reaches a final state. */
export function IntentProgress({ initial }: { initial: Intent }) {
  const { t } = useI18n();
  const [intent, setIntent] = useState(initial);
  const final = isFinalIntentStatus(intent.status);

  usePoll(!final, INTENT_POLL_MS, async () => {
    setIntent(await api.intent(intent.id));
  });

  return (
    <Card title={t.progress.title}>
      <ol className="space-y-4" aria-live="polite">
        {intentSteps(intent).map((step, i) => (
          <li key={step.key} className="flex gap-3" data-state={step.state}>
            <span aria-hidden className={`flex size-6 shrink-0 items-center justify-center rounded-full text-xs ${MARK[step.state]}`}>
              {step.state === "done" ? "✓" : step.state === "failed" ? "!" : i + 1}
            </span>
            <div className="space-y-0.5">
              <p className="text-sm font-medium">
                {t.progress.steps[step.key].label}
                <span className="sr-only">: {t.progress.state[step.state]}</span>
              </p>
              <p className="text-xs text-gray">{t.progress.steps[step.key].detail}</p>
              {step.hash && <TxLink hash={step.hash} />}
            </div>
          </li>
        ))}
      </ol>
      <div className="mt-5">
        {intent.status === "completed" && (
          <Notice tone="ok">{t.progress.completed(intent.amount_asset ?? "", intent.asset_code)}</Notice>
        )}
        {final && intent.status !== "completed" && (
          <Notice tone="error">{t.progress.ended(t.progress.status[intent.status] ?? intent.status, intent.message)}</Notice>
        )}
      </div>
    </Card>
  );
}
