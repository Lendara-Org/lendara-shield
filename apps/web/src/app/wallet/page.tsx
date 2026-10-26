import type { Metadata } from "next";

import { WalletView } from "@/components/wallet/wallet-view";

export const metadata: Metadata = { title: "Wallet" };

export default function WalletPage() {
  return (
    <div className="space-y-6">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold">Private wallet</h1>
        <p className="text-gray">Your balance in the pool, private transfers and withdrawals.</p>
      </header>
      <WalletView />
    </div>
  );
}
