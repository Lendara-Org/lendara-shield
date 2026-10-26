"use client";

import type { Intent } from "@lendara/shield-api-types";
import { useState } from "react";

import { message } from "@/components/session";
import { Button, Card, Field, Notice, inputClass } from "@/components/ui/primitives";
import { api } from "@/lib/api";

import { IntentProgress } from "./intent-progress";

const AMOUNT = /^(0|[1-9]\d*)(\.\d{1,2})?$/;
const CURRENCY = /^[A-Z]{3}$/;
const ZERO = /^0+(\.0+)?$/;

export function RampDeposit({ account, anchor }: { account: string; anchor: string | null | undefined }) {
  const [currency, setCurrency] = useState("CRC");
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [intent, setIntent] = useState<Intent | null>(null);

  const valid = CURRENCY.test(currency) && AMOUNT.test(amount) && !ZERO.test(amount);

  async function start(event: React.FormEvent) {
    event.preventDefault();
    if (!valid) return;
    setBusy(true);
    setError(null);
    // Opened in the click so the browser does not block it as a popup.
    const popup = window.open("about:blank", "shield-anchor", "popup,width=480,height=760");
    try {
      const created = await api.createIntent({
        mode: "ramp",
        stellar_account: account,
        fiat_currency: currency,
        amount,
      });
      setIntent(created);
      if (created.interactive_url) {
        if (popup) popup.location.href = created.interactive_url;
      } else {
        popup?.close();
      }
    } catch (err) {
      popup?.close();
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  if (intent) {
    return (
      <div className="space-y-4">
        {intent.interactive_url && (
          <Notice>
            Complete the payment in the anchor&apos;s window.{" "}
            <a href={intent.interactive_url} target="_blank" rel="noreferrer" className="font-medium underline">
              Reopen it
            </a>
          </Notice>
        )}
        <IntentProgress initial={intent} />
      </div>
    );
  }

  return (
    <Card title="Pay in local currency">
      <p className="mb-4 text-sm text-gray">
        The anchor{anchor ? ` (${anchor})` : ""} pays Shield&apos;s account with a memo. Shield deposits and transfers the USDC to
        your keys privately. Nothing on-chain points to your address.
      </p>
      <form onSubmit={(e) => void start(e)} className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-[8rem_1fr]">
          <Field label="Currency">
            {(control) => (
              <input
                {...control}
                className={inputClass}
                value={currency}
                maxLength={3}
                onChange={(e) => setCurrency(e.target.value.toUpperCase())}
                aria-invalid={!CURRENCY.test(currency)}
              />
            )}
          </Field>
          <Field label="Amount" hint="The anchor quotes it in USDC.">
            {(control) => (
              <input
                {...control}
                className={inputClass}
                inputMode="decimal"
                placeholder="5000"
                value={amount}
                onChange={(e) => setAmount(e.target.value.trim())}
              />
            )}
          </Field>
        </div>
        <Button type="submit" busy={busy} disabled={!valid}>
          Continue to the anchor
        </Button>
        {error && <Notice tone="error">{error}</Notice>}
      </form>
    </Card>
  );
}
