import type { Metadata } from "next";
import { Suspense } from "react";

import { DepositFlow } from "@/components/deposit/deposit-flow";
import { Page } from "@/components/ui/page";
import { type LangParams, pageCopy } from "@/lib/page-params";

export async function generateMetadata(props: LangParams): Promise<Metadata> {
  const { t } = await pageCopy(props);
  return { title: t.deposit.title };
}

export default async function DepositPage(props: LangParams) {
  const { t } = await pageCopy(props);
  return (
    <Page title={t.deposit.title} intro={t.deposit.intro}>
      <Suspense>
        <DepositFlow />
      </Suspense>
    </Page>
  );
}
