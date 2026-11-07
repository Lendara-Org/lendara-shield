import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { I18nProvider } from "@/components/i18n";
import { SessionProvider } from "@/components/session";
import type { Lang } from "@/lib/i18n";
import type { ShieldWallet } from "@/lib/pool/types";

import { connectorFor, fakeWallet } from "./fake-wallet";

/** Renders `ui` inside a session whose connector yields `wallet`, before the user connects. */
export function renderUnconnected(ui: React.ReactNode, wallet: ShieldWallet = fakeWallet(), lang: Lang = "en") {
  const user = userEvent.setup();
  const view = render(
    <I18nProvider lang={lang}>
      <SessionProvider connectorFactory={connectorFor(wallet)}>{ui}</SessionProvider>
    </I18nProvider>,
  );
  return { user, ...view };
}

/** Renders `ui` inside a session and connects `wallet` like a user would. */
export async function renderConnected(ui: React.ReactNode, wallet: ShieldWallet, lang: Lang = "en") {
  const { user, ...view } = renderUnconnected(ui, wallet, lang);
  const button = await screen.findByRole("button", { name: lang === "es" ? "Conectar Freighter" : "Connect Freighter" });
  await vi.waitFor(() => expect(button).toBeEnabled());
  await user.click(button);
  return { user, ...view };
}
