"use client";

import type { AuditTransaction } from "@lendara/shield-api-types";
import { useState } from "react";

import { useI18n } from "@/components/i18n";
import { message, useSession } from "@/components/session";
import { Button, Card, Field, Notice, inputClass } from "@/components/ui/primitives";
import { ApiRequestError, api } from "@/lib/api";
import { movedAmount } from "@/lib/audit";
import { shorten } from "@/lib/explorer";

import { Comparison } from "./comparison";

export function AuditView() {
  const { info } = useSession();
  const { t } = useI18n();
  const a = t.audit;
  const asset = info?.pool.asset.code ?? "USDC";
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [txs, setTxs] = useState<AuditTransaction[] | null>(null);
  const [selected, setSelected] = useState(0);

  async function unlock(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const { transactions } = await api.auditTransactions(token.trim());
      setTxs(transactions);
      setSelected(0);
    } catch (err) {
      setError(err instanceof ApiRequestError && err.status === 401 ? a.badToken : message(err));
    } finally {
      setBusy(false);
    }
  }

  if (!txs) {
    return (
      <Card title={a.accessTitle}>
        <form onSubmit={(e) => void unlock(e)} className="max-w-3xl space-y-6">
          <Field label={a.token} hint={a.tokenHint}>
            {(control) => (
              <input
                {...control}
                type="password"
                autoComplete="off"
                className={inputClass}
                value={token}
                onChange={(e) => setToken(e.target.value)}
              />
            )}
          </Field>
          <Button type="submit" busy={busy} disabled={token.trim() === ""}>
            {a.open}
          </Button>
          {error && <Notice tone="error">{error}</Notice>}
        </form>
      </Card>
    );
  }

  const current = txs[selected];
  return (
    <div className="space-y-6">
      {current ? <Comparison tx={current} asset={asset} /> : <Notice>{a.empty}</Notice>}
      <Card title={a.activity}>
        <ul className="divide-y divide-line">
          {txs.map((tx, i) => (
            <li key={`${tx.ledger}:${tx.tx_hash ?? i}`}>
              <button
                type="button"
                onClick={() => setSelected(i)}
                aria-current={i === selected}
                className="grid w-full grid-cols-[minmax(0,1fr)_auto] gap-3 py-4 sm:grid-cols-[minmax(0,11rem)_minmax(0,1fr)_auto] text-left text-base hover:bg-paper aria-[current=true]:font-semibold"
              >
                <span>{a.kinds[tx.kind]}</span>
                <span className="min-w-0 break-all font-mono text-gray">{tx.tx_hash ? shorten(tx.tx_hash) : a.ledger(tx.ledger)}</span>
                <span className="col-span-2 font-sans tabular-nums sm:col-span-1">
                  {movedAmount(tx)} {asset}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </Card>
    </div>
  );
}
