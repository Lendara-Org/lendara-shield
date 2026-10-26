import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SessionProvider } from "@/components/session";
import type { ShieldWallet } from "@/lib/pool/types";

import { connectorFor } from "./fake-wallet";

/** Renders `ui` inside a session and connects `wallet` like a user would. */
export async function renderConnected(ui: React.ReactNode, wallet: ShieldWallet) {
  const user = userEvent.setup();
  const view = render(<SessionProvider connectorFactory={connectorFor(wallet)}>{ui}</SessionProvider>);
  const button = await screen.findByRole("button", { name: "Connect Freighter" });
  await vi.waitFor(() => expect(button).toBeEnabled());
  await user.click(button);
  return { user, ...view };
}
