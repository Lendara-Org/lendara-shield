/** Wording of the side-by-side audit comparison. Pure, so it is tested. */
import type { AuditNote, AuditTransaction, PublicTransaction } from "@lendara/shield-api-types";

import { formatAmount, parseAmount } from "./amount";
import { shorten } from "./explorer";

const KIND: Record<AuditTransaction["kind"], string> = {
  deposit: "Deposit",
  transfer: "Private transfer",
  withdraw: "Withdraw",
};

export function kindLabel(kind: AuditTransaction["kind"]): string {
  return KIND[kind];
}

/** The public amount as the explorer shows it. */
export function publicAmount(ext: string | null, asset: string): string {
  if (ext === null) return "Not a pool transaction";
  if (ext === "0") return "Nothing: amount hidden";
  return ext.startsWith("-") ? `${ext.slice(1)} ${asset} out of the pool` : `${ext} ${asset} into the pool`;
}

/** Who a note belongs to, for the auditor. */
export function owner(note: AuditNote): string {
  return note.owner_stellar_account
    ? shorten(note.owner_stellar_account, 6, 4)
    : `Unknown key ${shorten(note.owner_note_public_key, 8, 4)}`;
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
