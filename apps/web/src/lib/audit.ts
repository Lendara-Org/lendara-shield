/** Wording of the side-by-side audit comparison. Pure, so it is tested. */
import type { AuditNote, AuditTransaction, PublicTransaction } from "@lendara/shield-api-types";

import type { Messages } from "@/content/messages";

import { formatAmount, parseAmount } from "./amount";
import { shorten } from "./explorer";

type AuditCopy = Messages["audit"];

/** The public amount as the explorer shows it. */
export function publicAmount(t: AuditCopy, ext: string | null, asset: string): string {
  if (ext === null) return t.notPool;
  if (ext === "0") return t.amountHidden;
  return ext.startsWith("-") ? t.amountOut(ext.slice(1), asset) : t.amountIn(ext, asset);
}

/** Who a note belongs to, for the auditor. */
export function owner(t: AuditCopy, note: AuditNote): string {
  return note.owner_stellar_account
    ? shorten(note.owner_stellar_account, 6, 4)
    : t.unknownKey(shorten(note.owner_note_public_key, 8, 4));
}

/** Accounts the explorer shows besides the signer (e.g. an asset issuer). */
export function otherVisible(tx: PublicTransaction): string[] {
  return tx.visible_accounts.filter((account) => account !== tx.source_account);
}

/** An output back to one of the spenders: the change of a transfer. */
export function isChange(tx: AuditTransaction, note: AuditNote): boolean {
  return tx.inputs.some((input) => input.owner_note_public_key === note.owner_note_public_key);
}

/** What the operation moved: the public amount, or what a transfer sent to others. */
export function movedAmount(tx: AuditTransaction): string {
  if (tx.kind !== "transfer") return tx.ext_amount.replace(/^-/, "");
  const sent = tx.outputs
    .filter((note) => !isChange(tx, note))
    .reduce((sum, note) => sum + (parseAmount(note.amount) ?? 0n), 0n);
  return formatAmount(sent);
}
