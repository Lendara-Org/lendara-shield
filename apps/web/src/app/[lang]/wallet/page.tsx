import type { Metadata } from "next";

import { WalletView } from "@/components/wallet/wallet-view";
import { Page } from "@/components/ui/page";
import { type LangParams, pageCopy } from "@/lib/page-params";

export async function generateMetadata(props: LangParams): Promise<Metadata> {
  const { t } = await pageCopy(props);
  return { title: t.wallet.title };
}

export default async function WalletPage(props: LangParams) {
  const { t } = await pageCopy(props);
  return (
    <Page title={t.wallet.title} intro={t.wallet.intro}>
      <WalletView />
    </Page>
  );
}
