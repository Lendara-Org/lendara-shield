"use client";

import { useI18n } from "@/components/i18n";
import { message } from "@/components/session";
import { Notice, TxList } from "@/components/ui/primitives";
import { isUserRejection } from "@/lib/pool/errors";
import { PoolOperationError } from "@/lib/pool/types";

/**
 * Explains a failed pool operation in the page's language, with the SDK's
 * text only when there is nothing better, and lists any step that was
 * already sent so the user knows what reached the network.
 */
export function PoolErrorNotice({ error, asset }: { error: unknown; asset: string }) {
  const { t } = useI18n();
  const kind = error instanceof PoolOperationError ? error.kind : isUserRejection(error) ? "rejected" : "other";
  const text = kind === "other" ? message(error) : t.poolError[kind](asset);
  const sent = error instanceof PoolOperationError ? error.hashes : [];
  return (
    <Notice tone="error">
      <p>{text}</p>
      {sent.length > 0 && (
        <div className="mt-2">
          <p className="mb-1">{t.poolError.sentBefore}</p>
          <TxList hashes={[...sent]} />
        </div>
      )}
    </Notice>
  );
}
