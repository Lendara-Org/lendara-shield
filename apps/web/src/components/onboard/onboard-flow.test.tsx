import { screen, within } from "@testing-library/react";
import { HttpResponse, http } from "msw";

import { fakeWallet } from "@/test/fake-wallet";
import { ACCOUNT, ENC_KEY, LEAF, NOTE_KEY, user } from "@/test/fixtures";
import { renderConnected } from "@/test/render";
import { apiError, server, url } from "@/test/server";

import { OnboardFlow } from "./onboard-flow";

vi.mock("@/lib/config", async (original) => ({ ...(await original<object>()), INTENT_POLL_MS: 10 }));

const newUser = () => server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => apiError(404, "not_found", "none")));

describe("OnboardFlow", () => {
  it("sends only public keys and the chosen scenario, then waits for the allow-list", async () => {
    newUser();
    let body: unknown;
    server.use(
      http.post(url("/v1/onboarding"), async ({ request }) => {
        body = await request.json();
        server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => HttpResponse.json(user())));
        return HttpResponse.json(user({ asp_status: "pending", last_asp_tx_hash: null }), { status: 201 });
      }),
    );
    const wallet = fakeWallet();
    const { user: ui } = await renderConnected(<OnboardFlow />, wallet);

    await ui.selectOptions(await screen.findByLabelText("KYC scenario"), "blocked");
    await ui.click(screen.getByRole("button", { name: "Derive keys and register" }));

    expect(body).toEqual({
      stellar_account: ACCOUNT,
      note_public_key: NOTE_KEY,
      encryption_public_key: ENC_KEY,
      asp_leaf: LEAF,
      scenario: "blocked",
    });
    expect(await screen.findByText(/Updating the pool's allow-list/)).toBeInTheDocument();
    expect(await screen.findByText(/You are in the allow-list/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Continue to deposit/ })).toHaveAttribute("href", "/deposit");
  });

  it("shows an existing user's status and ASP transaction", async () => {
    server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => HttpResponse.json(user({ kyc_status: "rejected", asp_status: "not_allowed", last_asp_tx_hash: null }))));
    await renderConnected(<OnboardFlow />, fakeWallet());

    expect(await screen.findByText(/KYC rejected/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /register/ })).not.toBeInTheDocument();
  });

  it("links the ASP update to the explorer", async () => {
    server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => HttpResponse.json(user())));
    await renderConnected(<OnboardFlow />, fakeWallet());
    const status = await screen.findByRole("heading", { name: "Your Shield status" });
    const link = within(status.parentElement ?? document.body).getByRole("link", { name: /17c2/ });
    expect(link.getAttribute("href")).toMatch(/^https:\/\/stellar\.expert\/explorer\/testnet\/tx\/17c2/);
  });

  it("shows why registering failed and lets the user retry", async () => {
    newUser();
    server.use(http.post(url("/v1/onboarding"), () => apiError(409, "keys_conflict", "The account is registered with other keys")));
    const { user: ui } = await renderConnected(<OnboardFlow />, fakeWallet());

    await ui.click(await screen.findByRole("button", { name: "Derive keys and register" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("registered with other keys");
    expect(screen.getByRole("button", { name: "Derive keys and register" })).toBeEnabled();
  });

  it("does not call Shield when the wallet declines to derive keys", async () => {
    newUser();
    const onboard = vi.fn();
    server.use(http.post(url("/v1/onboarding"), onboard));
    const wallet = fakeWallet({ keys: () => Promise.reject(new Error("User declined to sign")) });
    const { user: ui } = await renderConnected(<OnboardFlow />, wallet);

    await ui.click(await screen.findByRole("button", { name: "Derive keys and register" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("User declined to sign");
    expect(onboard).not.toHaveBeenCalled();
  });
});
