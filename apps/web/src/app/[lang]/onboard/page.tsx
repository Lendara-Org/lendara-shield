import type { Metadata } from "next";

import { OnboardFlow } from "@/components/onboard/onboard-flow";
import { Page } from "@/components/ui/page";
import { type LangParams, pageCopy } from "@/lib/page-params";

export async function generateMetadata(props: LangParams): Promise<Metadata> {
  const { t } = await pageCopy(props);
  return { title: t.onboard.title };
}

export default async function OnboardPage(props: LangParams) {
  const { t } = await pageCopy(props);
  return (
    <Page title={t.onboard.title} intro={t.onboard.intro}>
      <OnboardFlow />
    </Page>
  );
}
