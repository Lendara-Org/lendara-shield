import type { Metadata } from "next";
import { Suspense } from "react";

import { DepositFlow } from "@/components/deposit/deposit-flow";

export const metadata: Metadata = { title: "Deposit" };

export default function DepositPage() {
  return (
    <div className="space-y-6">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold">Deposit</h1>
        <p className="text-gray">Move money into your private balance.</p>
      </header>
      <Suspense>
        <DepositFlow />
      </Suspense>
    </div>
  );
}
