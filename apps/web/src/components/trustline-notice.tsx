"use client";

import { useI18n } from "@/components/i18n";
import { Button, Notice } from "@/components/ui/primitives";
import type { Trustline } from "@/lib/hooks/use-trustline";
import type { ClassicAsset } from "@/lib/trustline";

/** Tells the user to add the asset's trustline in Freighter when it is missing; nothing otherwise. */
export function TrustlineNotice({ trust, asset }: { trust: Trustline; asset: ClassicAsset }) {
  const { t } = useI18n();
  if (trust.state !== "missing") return null;
  return (
    <Notice tone="error">
      <p>{t.trustline.missing(asset.code)}</p>
      <p className="mt-2">{t.trustline.howTo(asset.code)}</p>
      <code className="mt-2 block break-all font-mono text-sm">{asset.issuer}</code>
      <div className="mt-3">
        <Button type="button" variant="quiet" onClick={() => void trust.recheck()}>
          {t.trustline.recheck}
        </Button>
      </div>
    </Notice>
  );
}
