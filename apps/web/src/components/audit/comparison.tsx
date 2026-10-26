"use client";

import type { AuditNote, AuditTransaction, PublicTransaction } from "@lendara/shield-api-types";
import { useEffect, useState } from "react";

import { useI18n } from "@/components/i18n";
import { message } from "@/components/session";
import { Card, Notice, TxLink } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { isChange, otherVisible, owner, publicAmount } from "@/lib/audit";
import { shorten } from "@/lib/explorer";

type PublicView = { tx: PublicTransaction } | { error: string } | null;

/** One transaction, side by side: the explorer's view and the auditor's. */
export function Comparison({ tx, asset }: { tx: AuditTransaction; asset: string }) {
  const { t } = useI18n();
  const a = t.audit;
  const [view, setView] = useState<PublicView>(null);
  const hash = tx.tx_hash;

  useEffect(() => {
    if (!hash) return;
    let active = true;
    api.publicTransaction(hash).then(
      (value) => active && setView({ tx: value }),
      (err: unknown) => active && setView({ error: message(err) }),
    );
    return () => {
      active = false;
    };
  }, [hash]);

  const current = view && "tx" in view && view.tx.tx_hash === hash ? view.tx : null;

  return (
    <div className="grid gap-4 md:grid-cols-2">
      <Card title={a.explorerTitle}>
        {!hash && <Notice>{a.unresolved}</Notice>}
        {hash && view && "error" in view && <Notice tone="error">{view.error}</Notice>}
        {hash && !current && !(view && "error" in view) && <p className="text-sm text-gray">{a.loading}</p>}
        {current && (
          <dl className="space-y-3 text-sm">
            <Row label={a.signedBy}>
              <span className="font-mono">{shorten(current.source_account, 6, 4)}</span>
            </Row>
            <Row label={a.publicAmount}>{publicAmount(a, current.ext_amount, asset)}</Row>
            <Row label={a.recipient}>{a.notVisible}</Row>
            {otherVisible(current).length > 0 && (
              <Row label={a.alsoVisible}>
                <span className="font-mono">{otherVisible(current).map((a) => shorten(a, 6, 4)).join(", ")}</span>
                <span className="block text-xs text-gray">{a.issuerNote}</span>
              </Row>
            )}
            <Row label={a.transaction}>
              <TxLink hash={current.tx_hash} />
            </Row>
          </dl>
        )}
      </Card>

      <Card title={a.auditorTitle} className="border-ink/30">
        <dl className="space-y-3 text-sm">
          <Row label={a.operation}>
            {a.kinds[tx.kind]} · {a.ledger(tx.ledger)}
          </Row>
          <Row label={a.from}>
            <Notes tx={tx} notes={tx.inputs} asset={asset} empty={tx.kind === "deposit" ? a.publicDeposit : "—"} />
          </Row>
          <Row label={a.to}>
            <Notes tx={tx} notes={tx.outputs} asset={asset} empty={tx.kind === "withdraw" ? a.publicWithdrawal : "—"} />
          </Row>
        </dl>
        <p className="mt-4 text-xs text-gray">{a.gvkNote}</p>
      </Card>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[7.5rem_1fr] gap-2">
      <dt className="text-gray">{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function Notes({ tx, notes, asset, empty }: { tx: AuditTransaction; notes: AuditNote[]; asset: string; empty: string }) {
  const { t } = useI18n();
  if (notes.length === 0) return <span className="text-gray">{empty}</span>;
  return (
    <ul className="space-y-1">
      {notes.map((note) => (
        <li key={`${note.owner_note_public_key}:${note.amount}`}>
          <span className="font-mono">{owner(t.audit, note)}</span> · {note.amount} {asset}
          {notes === tx.outputs && isChange(tx, note) && <span className="text-gray"> {t.audit.change}</span>}
        </li>
      ))}
    </ul>
  );
}
