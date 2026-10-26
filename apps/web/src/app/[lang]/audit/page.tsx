import type { Metadata } from "next";

import { AuditView } from "@/components/audit/audit-view";
import { Page } from "@/components/ui/page";
import { type LangParams, pageCopy } from "@/lib/page-params";

export async function generateMetadata(props: LangParams): Promise<Metadata> {
  const { t } = await pageCopy(props);
  return { title: t.audit.title };
}

export default async function AuditPage(props: LangParams) {
  const { t } = await pageCopy(props);
  return (
    <Page title={t.audit.title} intro={t.audit.intro}>
      <AuditView />
    </Page>
  );
}
