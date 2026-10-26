"use client";

import { useSearchParams } from "next/navigation";
import { useState } from "react";

import { ConnectGate } from "@/components/connect-gate";
import { useI18n } from "@/components/i18n";
import { useSession } from "@/components/session";

import { DirectDeposit } from "./direct-deposit";
import { RampDeposit } from "./ramp-deposit";

type Mode = "ramp" | "direct";

export function DepositFlow() {
  const { info } = useSession();
  const { t } = useI18n();
  const params = useSearchParams();
  const rampEnabled = info?.modes.ramp.enabled ?? false;
  const [chosen, setChosen] = useState<Mode>(params.get("mode") === "direct" ? "direct" : "ramp");
  const mode: Mode = rampEnabled ? chosen : "direct";

  return (
    <ConnectGate requireAllowed>
      {(wallet) => (
        <div className="space-y-6">
          <div role="radiogroup" aria-label={t.deposit.mode} className="inline-flex rounded-md border border-line bg-surface p-1 text-sm">
            {(
              [
                ["ramp", t.deposit.ramp],
                ["direct", t.deposit.direct],
              ] as const
            ).map(([value, label]) => (
              <button
                key={value}
                type="button"
                role="radio"
                aria-checked={mode === value}
                disabled={value === "ramp" && !rampEnabled}
                onClick={() => setChosen(value)}
                className="rounded px-4 py-1.5 text-gray aria-checked:bg-ink aria-checked:text-paper disabled:opacity-40"
              >
                {label}
              </button>
            ))}
          </div>
          {mode === "ramp" ? (
            <RampDeposit account={wallet.address} anchor={info?.modes.ramp.anchor_home_domain} />
          ) : (
            <DirectDeposit wallet={wallet} assetCode={info?.pool.asset.code ?? "USDC"} />
          )}
        </div>
      )}
    </ConnectGate>
  );
}
