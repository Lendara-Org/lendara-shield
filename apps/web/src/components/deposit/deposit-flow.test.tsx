import { screen, within } from "@testing-library/react";
import { HttpResponse, http } from "msw";

import { SOROBAN_RPC_URL } from "@/lib/config";
import { intentMessage } from "@/lib/intent-request";
import { fakeWallet } from "@/test/fake-wallet";
import { ACCOUNT, HASH, completed, info, intent, user } from "@/test/fixtures";
import { renderConnected, renderUnconnected } from "@/test/render";
import { apiError, rpcTrustline, server, url } from "@/test/server";

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
    const wallet = fakeWallet();
    const { user: ui } = await renderConnected(<DepositFlow />, wallet);

    await ui.type(await screen.findByLabelText("Amount"), "5000");
    await ui.click(screen.getByRole("button", { name: "Continue to the anchor" }));

    expect(body).toEqual({
      mode: "ramp",
      stellar_account: ACCOUNT,
      fiat_currency: "CRC",
      amount: "5000",
      request_id: expect.stringMatching(/^[0-9a-f-]{36}$/),
      issued_at: expect.any(Number),
      signature: "c2lnbmVkLWJ5LXRoZS1hY2NvdW50",
    });
    const sent = body as { request_id: string; issued_at: number };
    expect(wallet.signMessage).toHaveBeenCalledWith(intentMessage(ACCOUNT, "CRC", "5000", sent.request_id, sent.issued_at));
    expect(anchor.location.href).toBe(intent().interactive_url);
    expect(await screen.findByText(/9.3039215 USDC are now private/)).toBeInTheDocument();
    const steps = screen.getAllByRole("listitem");
    expect(steps.map((s) => s.dataset.state)).toEqual(["done", "done", "done", "done"]);
    for (const hash of ["2fc5", "d43d", "5dd9"]) {
      expect(within(screen.getByRole("list")).getByRole("link", { name: new RegExp(hash) })).toBeInTheDocument();
    }
  });

  it("starts nothing when the user declines to sign", async () => {
    let posted = false;
    server.use(
      http.post(url("/v1/intents"), () => {
        posted = true;
        return HttpResponse.json(intent(), { status: 201 });
      }),
    );
    const anchor = popup();
    const wallet = fakeWallet({ signMessage: vi.fn(() => Promise.reject(new Error("User declined"))) });
    const { user: ui } = await renderConnected(<DepositFlow />, wallet);

    await ui.type(await screen.findByLabelText("Amount"), "5000");
    await ui.click(screen.getByRole("button", { name: "Continue to the anchor" }));

    expect(await screen.findByText("User declined")).toBeInTheDocument();
    expect(posted).toBe(false);
    expect(anchor.close).toHaveBeenCalled();
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

    expect(await screen.findByRole("alert")).toHaveTextContent("Deposit failed: The anchor rejected the KYC.");
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

describe("DepositFlow before connecting", () => {
  it("recommends the anchor's ramp, which needs no USDC, and hides it once connected", async () => {
    const { user: ui } = renderUnconnected(<DepositFlow />);
    const options = await screen.findByRole("heading", { name: "Two ways to deposit" });
    const card = options.closest("section") ?? document.body;
    expect(card).toHaveTextContent("through the anchor lendara-monorepo-fixh.vercel.app (recommended)");
    expect(card).toHaveTextContent("You need no USDC and no trustline");
    expect(card).toHaveTextContent("you need the Lendara sandbox USDC and its trustline");
    await ui.click(screen.getByRole("button", { name: "Connect Freighter" }));
    await vi.waitFor(() => expect(screen.queryByRole("heading", { name: "Two ways to deposit" })).not.toBeInTheDocument());
  });

  it("does not offer the ramp when ramp mode is disabled", async () => {
    server.use(
      http.get(url("/v1/info"), () =>
        HttpResponse.json({ ...info, modes: { ...info.modes, ramp: { enabled: false, anchor_home_domain: null } } }),
      ),
    );
    renderUnconnected(<DepositFlow />);
    await vi.waitFor(() => expect(screen.getByRole("button", { name: "Connect Freighter" })).toBeEnabled());
    expect(screen.queryByRole("heading", { name: "Two ways to deposit" })).not.toBeInTheDocument();
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

  it("asks for the USDC trustline and deposits once it is added", async () => {
    searchParams = new URLSearchParams("mode=direct");
    server.use(rpcTrustline(false));
    const wallet = fakeWallet();
    const { user: ui } = await renderConnected(<DepositFlow />, wallet);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Your account has no USDC trustline");
    expect(alert).toHaveTextContent(info.pool.asset.issuer);
    expect(alert).toHaveTextContent("only takes the Lendara sandbox USDC");
    expect(alert).toHaveTextContent("Depositing with a ramp needs no trustline");
    await ui.type(screen.getByLabelText("Amount (USDC)"), "1.5");
    const submit = screen.getByRole("button", { name: "Deposit into the pool" });
    expect(submit).toBeDisabled();

    server.use(rpcTrustline(true));
    await ui.click(screen.getByRole("button", { name: "Check again" }));
    await vi.waitFor(() => expect(submit).toBeEnabled());
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await ui.click(submit);
    expect(wallet.deposit).toHaveBeenCalledWith(15_000_000n);
  });

  it("does not block the deposit when the RPC can't tell", async () => {
    searchParams = new URLSearchParams("mode=direct");
    server.use(http.post(SOROBAN_RPC_URL, () => new HttpResponse(null, { status: 503 })));
    const wallet = fakeWallet();
    const { user: ui } = await renderConnected(<DepositFlow />, wallet);
    await ui.type(await screen.findByLabelText("Amount (USDC)"), "1");
    await ui.click(screen.getByRole("button", { name: "Deposit into the pool" }));
    expect(wallet.deposit).toHaveBeenCalledWith(10_000_000n);
    expect(screen.queryByText(/trustline/)).not.toBeInTheDocument();
  });
});
