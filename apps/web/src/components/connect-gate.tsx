"use client";

import Link from "next/link";

import type { ShieldWallet } from "@/lib/pool/types";

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

  if (infoError) return <Notice tone="error">{infoError}</Notice>;
  if (!wallet) {
    return (
      <Card title="Connect your wallet">
        <p className="mb-4 text-sm text-gray">
          Shield uses Freighter on Testnet. Your pool keys are derived in this browser and never leave it.
        </p>
        <div className="space-y-3">
          <Button onClick={() => void connect()} busy={connecting} disabled={!info}>
            Connect Freighter
          </Button>
          {connectError && <Notice tone="error">{connectError}</Notice>}
        </div>
      </Card>
    );
  }
  if (requireAllowed && user?.asp_status !== "allowed") {
    return (
      <Notice tone="warn">
        {user ? `Your key is ${user.asp_status.replace("_", " ")} in Shield's allow-list.` : "You have not onboarded yet."}{" "}
        <Link href="/onboard" className="font-medium underline">
          Go to onboarding
        </Link>
      </Notice>
    );
  }
  return <>{children(wallet)}</>;
}
