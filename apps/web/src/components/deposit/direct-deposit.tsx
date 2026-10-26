"use client";

import { useState } from "react";

import { useI18n } from "@/components/i18n";
import { message } from "@/components/session";
import { Button, Card, Field, Notice, TxList, inputClass } from "@/components/ui/primitives";
import { parseAmount } from "@/lib/amount";
import type { PoolResult, ShieldWallet } from "@/lib/pool/types";

export function DirectDeposit({ wallet, assetCode }: { wallet: ShieldWallet; assetCode: string }) {
  const { t } = useI18n();
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<PoolResult | null>(null);
  const stroops = parseAmount(amount);

  async function deposit(event: React.FormEvent) {
    event.preventDefault();
    if (stroops === null) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await wallet.deposit(stroops));
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card title={t.deposit.directTitle(assetCode)}>
      <div className="mb-4">
        <Notice tone="warn">{t.deposit.directWarning}</Notice>
      </div>
      <form onSubmit={(e) => void deposit(e)} className="max-w-3xl space-y-6">
        <Field label={t.deposit.directAmount(assetCode)} hint={t.deposit.directHint}>
          {(control) => (
            <input
              {...control}
              className={inputClass}
              inputMode="decimal"
              placeholder="1.5"
              value={amount}
              onChange={(e) => setAmount(e.target.value.trim())}
            />
          )}
        </Field>
        <Button type="submit" busy={busy} disabled={stroops === null}>
          {t.deposit.depositButton}
        </Button>
        {error && <Notice tone="error">{error}</Notice>}
        {result && (
          <Notice tone="ok">
            <p className="mb-1">{t.deposit.deposited}</p>
            <TxList hashes={result.hashes} />
          </Notice>
        )}
      </form>
    </Card>
  );
}
