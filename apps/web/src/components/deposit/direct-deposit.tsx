"use client";

import { useState } from "react";

import { useI18n } from "@/components/i18n";
import { PoolErrorNotice } from "@/components/pool-error";
import { TrustlineNotice } from "@/components/trustline-notice";
import { Button, Card, Field, Notice, TxList, inputClass } from "@/components/ui/primitives";
import { parseAmount } from "@/lib/amount";
import { useTrustline } from "@/lib/hooks/use-trustline";
import type { PoolResult, ShieldWallet } from "@/lib/pool/types";
import type { ClassicAsset } from "@/lib/trustline";

export function DirectDeposit({
  wallet,
  asset,
  rampEnabled,
}: {
  wallet: ShieldWallet;
  asset: ClassicAsset | undefined;
  rampEnabled: boolean;
}) {
  const { t } = useI18n();
  const assetCode = asset?.code ?? "USDC";
  const trust = useTrustline(wallet.address, asset);
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [result, setResult] = useState<PoolResult | null>(null);
  const stroops = parseAmount(amount);

  async function deposit(event: React.FormEvent) {
    event.preventDefault();
    if (stroops === null || trust.state === "missing") return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await wallet.deposit(stroops));
    } catch (err) {
      setError(err);
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
        {asset && <TrustlineNotice trust={trust} asset={asset} showRamp={rampEnabled} />}
        <Button type="submit" busy={busy} disabled={stroops === null || trust.state === "missing"}>
          {t.deposit.depositButton}
        </Button>
        {error !== null && <PoolErrorNotice error={error} asset={assetCode} />}
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
