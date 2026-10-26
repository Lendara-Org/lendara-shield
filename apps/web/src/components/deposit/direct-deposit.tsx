"use client";

import { useState } from "react";

import { message } from "@/components/session";
import { Button, Card, Field, Notice, TxList, inputClass } from "@/components/ui/primitives";
import { parseAmount } from "@/lib/amount";
import type { PoolResult, ShieldWallet } from "@/lib/pool/types";

export function DirectDeposit({ wallet, assetCode }: { wallet: ShieldWallet; assetCode: string }) {
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
    <Card title={`Deposit ${assetCode} from your wallet`}>
      <div className="mb-4">
        <Notice tone="warn">
          Your address is visible on-chain for this deposit (you sign it). Transfers after it are private. To keep your
          address out entirely, deposit with a ramp.
        </Notice>
      </div>
      <form onSubmit={(e) => void deposit(e)} className="space-y-4">
        <Field label={`Amount (${assetCode})`} hint="Proving takes a few seconds; Freighter asks you to sign.">
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
          Deposit into the pool
        </Button>
        {error && <Notice tone="error">{error}</Notice>}
        {result && (
          <Notice tone="ok">
            <p className="mb-1">Deposited. Pool transaction (ext_amount &gt; 0):</p>
            <TxList hashes={result.hashes} />
          </Notice>
        )}
      </form>
    </Card>
  );
}
