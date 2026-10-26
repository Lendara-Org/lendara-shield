import type { Metadata } from "next";

import { AuditView } from "@/components/audit/audit-view";

export const metadata: Metadata = { title: "Audit" };

export default function AuditPage() {
  return (
    <div className="space-y-6">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold">Audit</h1>
        <p className="text-gray">Private to the public, accountable to the auditor: the same transaction, two views.</p>
      </header>
      <AuditView />
    </div>
  );
}
