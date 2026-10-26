import type { Metadata } from "next";

import { OnboardFlow } from "@/components/onboard/onboard-flow";

export const metadata: Metadata = { title: "Onboard" };

export default function OnboardPage() {
  return (
    <div className="space-y-6">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold">Onboard</h1>
        <p className="text-gray">KYC once: Shield vouches for your pool key without ever holding it.</p>
      </header>
      <OnboardFlow />
    </div>
  );
}
