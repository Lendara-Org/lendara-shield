import { useCallback, useEffect, useState } from "react";

import { type ClassicAsset, type TrustlineState, readTrustline } from "@/lib/trustline";

export interface Trustline {
  /** `checking` until the first read settles; `unknown` without an asset or when the RPC fails. */
  state: TrustlineState | "checking";
  /** Reads it again, e.g. after the user adds it in Freighter. */
  recheck: () => Promise<void>;
}

/** Whether `account` trusts `asset`, read when either changes. */
export function useTrustline(account: string, asset: ClassicAsset | undefined): Trustline {
  const code = asset?.code;
  const issuer = asset?.issuer;
  const [state, setState] = useState<Trustline["state"]>(code && issuer ? "checking" : "unknown");

  useEffect(() => {
    if (!code || !issuer) return;
    let active = true;
    void readTrustline(account, { code, issuer }).then((next) => active && setState(next));
    return () => {
      active = false;
    };
  }, [account, code, issuer]);

  const recheck = useCallback(async () => {
    if (!code || !issuer) return;
    setState("checking");
    setState(await readTrustline(account, { code, issuer }));
  }, [account, code, issuer]);

  return { state, recheck };
}
