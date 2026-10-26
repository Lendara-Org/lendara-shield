import { screen, within } from "@testing-library/react";
import { HttpResponse, http } from "msw";

import { fakeWallet } from "@/test/fake-wallet";
import { ACCOUNT, HASH, completed, info, intent, user } from "@/test/fixtures";
import { renderConnected } from "@/test/render";
import { apiError, server, url } from "@/test/server";

import { DepositFlow } from "./deposit-flow";

vi.mock("@/lib/config", async (original) => ({ ...(await original<object>()), INTENT_POLL_MS: 10 }));

let searchParams = new URLSearchParams();
vi.mock("next/navigation", () => ({ useSearchParams: () => searchParams }));

function popup() {
  const window_ = { location: { href: "" }, close: vi.fn() };
  vi.spyOn(window, "open").mockReturnValue(window_ as unknown as Window);
  return window_;
}

beforeEach(() => {
  searchParams = new URLSearchParams();
  server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => HttpResponse.json(user())));
});

describe("DepositFlow with a ramp", () => {
  it("creates an intent, opens the anchor and follows it to completed", async () => {
    let body: unknown;
    const statuses = [
      intent({ status: "funds_received", amount_asset: "9.3039215", transactions: { intake_payment: HASH("2fc5"), pool_deposit: null, pool_transfer: null } }),
      completed,
    ];
    server.use(
      http.post(url("/v1/intents"), async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(intent(), { status: 201 });
      }),
      http.get(url(`/v1/intents/${completed.id}`), () => HttpResponse.json(statuses.shift() ?? completed)),
    );
    const anchor = popup();
    const { user: ui } = await renderConnected(<DepositFlow />, fakeWallet());

    await ui.type(await screen.findByLabelText("Amount"), "5000");
    await ui.click(screen.getByRole("button", { name: "Continue to the anchor" }));

    expect(body).toEqual({ mode: "ramp", stellar_account: ACCOUNT, fiat_currency: "CRC", amount: "5000" });
    expect(anchor.location.href).toBe(intent().interactive_url);
    expect(await screen.findByText(/9.3039215 USDC are now private/)).toBeInTheDocument();
    const steps = screen.getAllByRole("listitem");
    expect(steps.map((s) => s.dataset.state)).toEqual(["done", "done", "done", "done"]);
    for (const hash of ["2fc5", "d43d", "5dd9"]) {
      expect(within(screen.getByRole("list")).getByRole("link", { name: new RegExp(hash) })).toBeInTheDocument();
    }
  });

  it("shows why the anchor step failed", async () => {
    server.use(
      http.post(url("/v1/intents"), () => HttpResponse.json(intent(), { status: 201 })),
      http.get(url(`/v1/intents/${completed.id}`), () =>
        HttpResponse.json(intent({ status: "error", interactive_url: null, message: "The anchor rejected the KYC." })),
      ),
    );
    popup();
    const { user: ui } = await renderConnected(<DepositFlow />, fakeWallet());
    await ui.type(await screen.findByLabelText("Amount"), "5000");
    await ui.click(screen.getByRole("button", { name: "Continue to the anchor" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Deposit error: The anchor rejected the KYC.");
    expect(screen.getAllByRole("listitem")[0]?.dataset.state).toBe("failed");
  });

  it("closes the popup and explains when Shield cannot start the deposit", async () => {
    server.use(http.post(url("/v1/intents"), () => apiError(502, "anchor_unavailable", "The anchor did not answer")));
    const anchor = popup();
    const { user: ui } = await renderConnected(<DepositFlow />, fakeWallet());
    await ui.type(await screen.findByLabelText("Amount"), "5000");
    await ui.click(screen.getByRole("button", { name: "Continue to the anchor" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("The anchor did not answer");
    expect(anchor.close).toHaveBeenCalled();
  });

  it("only submits a valid currency and amount", async () => {
    const { user: ui } = await renderConnected(<DepositFlow />, fakeWallet());
    const submit = await screen.findByRole("button", { name: "Continue to the anchor" });
    expect(submit).toBeDisabled();
    await ui.type(screen.getByLabelText("Amount"), "0");
    expect(submit).toBeDisabled();
    await ui.clear(screen.getByLabelText("Amount"));
    await ui.type(screen.getByLabelText("Amount"), "12.50");
    expect(submit).toBeEnabled();
    await ui.clear(screen.getByLabelText("Currency"));
    await ui.type(screen.getByLabelText("Currency"), "cr");
    expect(submit).toBeDisabled();
  });
});

describe("DepositFlow from the wallet", () => {
  it("warns that the address is visible and deposits stroops", async () => {
    searchParams = new URLSearchParams("mode=direct");
    const wallet = fakeWallet();
    const { user: ui } = await renderConnected(<DepositFlow />, wallet);

    expect(await screen.findByText(/Your address is visible on-chain/)).toBeInTheDocument();
    await ui.type(screen.getByLabelText("Amount (USDC)"), "1.5");
    await ui.click(screen.getByRole("button", { name: "Deposit into the pool" }));

    expect(wallet.deposit).toHaveBeenCalledWith(15_000_000n);
    expect(await screen.findByRole("link", { name: /dddddddd/ })).toBeInTheDocument();
  });

  it("is the only mode when the ramp is disabled", async () => {
    server.use(
      http.get(url("/v1/info"), () =>
        HttpResponse.json({ ...info, modes: { ...info.modes, ramp: { enabled: false, anchor_home_domain: null } } }),
      ),
    );
    await renderConnected(<DepositFlow />, fakeWallet());
    expect(await screen.findByRole("radio", { name: "With a ramp" })).toBeDisabled();
    expect(screen.getByRole("radio", { name: "From my wallet" })).toBeChecked();
  });

  it("reports a pool failure", async () => {
    searchParams = new URLSearchParams("mode=direct");
    const wallet = fakeWallet({ deposit: () => Promise.reject(new Error("The pool has not seen your allow-list entry yet.")) });
    const { user: ui } = await renderConnected(<DepositFlow />, wallet);
    await ui.type(await screen.findByLabelText("Amount (USDC)"), "1");
    await ui.click(screen.getByRole("button", { name: "Deposit into the pool" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("allow-list entry");
  });
});
