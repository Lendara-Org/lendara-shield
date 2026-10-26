"use client";

import Link from "next/link";

import type { ShieldWallet } from "@/lib/pool/types";

import { useI18n } from "./i18n";
import { useSession } from "./session";
import { Button, Card, Notice } from "./ui/primitives";

/** Renders `children` once a wallet is connected (and, if asked, allowed). */
export function ConnectGate({
  requireAllowed = false,
  children,
}: {
  requireAllowed?: boolean;
  children: (wallet: ShieldWallet) => React.ReactNode;
}) {
  const { info, infoError, wallet, user, connect, connecting, connectError } = useSession();
  const { t, href } = useI18n();

  if (infoError) return <Notice tone="error">{infoError}</Notice>;
  if (!wallet) {
    return (
      <Card title={t.connect.title}>
        <p className="mb-6 max-w-3xl text-lg leading-8 text-gray">{t.connect.intro}</p>
        <div className="space-y-3">
          <Button onClick={() => void connect()} busy={connecting} disabled={!info}>
            {t.connect.button}
          </Button>
          {connectError && <Notice tone="error">{connectError}</Notice>}
        </div>
      </Card>
    );
  }
  if (requireAllowed && user?.asp_status !== "allowed") {
    return (
      <Notice tone="warn">
        {user ? t.connect.keyStatus(t.aspStatus[user.asp_status]) : t.connect.notOnboarded}{" "}
        <Link href={href("/onboard")} className="font-medium underline">
          {t.connect.goOnboard}
        </Link>
      </Notice>
    );
  }
  return <>{children(wallet)}</>;
}
