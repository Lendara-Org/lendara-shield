import { screen } from "@testing-library/react";
import { HttpResponse, http } from "msw";

import { PoolOperationError } from "@/lib/pool/types";
import { fakeWallet } from "@/test/fake-wallet";
import { ACCOUNT, ENC_KEY, HASH, NOTE_KEY, user } from "@/test/fixtures";
import { renderConnected } from "@/test/render";
import { rpcTrustline, server, url } from "@/test/server";

import { WalletView } from "./wallet-view";

const RECIPIENT = { notePublicKey: `0x${"bb".repeat(32)}`, encryptionPublicKey: `0x${"cc".repeat(32)}` };
const recipientAddress = `shield:${RECIPIENT.notePublicKey}:${RECIPIENT.encryptionPublicKey}`;

beforeEach(() => {
  server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => HttpResponse.json(user())));
});

describe("WalletView", () => {
  it("shows the private balance", async () => {
    await renderConnected(<WalletView />, fakeWallet());
    expect(await screen.findByText("9.3039215")).toBeInTheDocument();
  });

  it("shows the receiving address built from public keys only", async () => {
    const { user: ui } = await renderConnected(<WalletView />, fakeWallet());
    await ui.click(await screen.findByRole("button", { name: "Show my private address" }));
    expect(await screen.findByText(`shield:${NOTE_KEY}:${ENC_KEY}`)).toBeInTheDocument();
  });

  it("sends privately to a valid address, then refreshes the balance", async () => {
    let balance = 93_039_215n;
    const wallet = fakeWallet({ balance: vi.fn(() => Promise.resolve(balance)) });
    wallet.transfer = vi.fn(() => {
      balance -= 10_000_000n;
      return Promise.resolve({ hashes: ["ee".repeat(32)] });
    });
    const { user: ui } = await renderConnected(<WalletView />, wallet);
    const send = (await screen.findAllByRole("button", { name: "Send" }))[0];
    if (!send) throw new Error("no send button");

    await ui.type(screen.getByLabelText("Recipient's private address"), "G-not-a-shield-address");
    await ui.type(screen.getAllByLabelText("Amount (USDC)")[0] ?? document.body, "1");
    expect(send).toBeDisabled();

    await ui.clear(screen.getByLabelText("Recipient's private address"));
    await ui.type(screen.getByLabelText("Recipient's private address"), recipientAddress);
    await ui.click(send);

    expect(wallet.transfer).toHaveBeenCalledWith(RECIPIENT, 10_000_000n);
    expect(await screen.findByText("8.3039215")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /eeeeeeee/ })).toBeInTheDocument();
  });

  it("withdraws to the connected wallet", async () => {
    const wallet = fakeWallet();
    const { user: ui } = await renderConnected(<WalletView />, wallet);
    const amounts = await screen.findAllByLabelText("Amount (USDC)");
    await ui.type(amounts[1] ?? document.body, "2.25");
    await ui.click(screen.getByRole("button", { name: "Withdraw" }));
    expect(wallet.withdraw).toHaveBeenCalledWith(22_500_000n);
  });

  it("blocks the withdraw, not the private transfer, without the USDC trustline", async () => {
    server.use(rpcTrustline(false));
    const { user: ui } = await renderConnected(<WalletView />, fakeWallet());
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Your account has no USDC trustline");
    expect(alert).toHaveTextContent("Circle's testnet USDC");
    // A withdraw has no ramp alternative.
    expect(alert).not.toHaveTextContent("ramp");
    const amounts = screen.getAllByLabelText("Amount (USDC)");
    await ui.type(amounts[1] ?? document.body, "2.25");
    expect(screen.getByRole("button", { name: "Withdraw" })).toBeDisabled();
    await ui.type(screen.getByLabelText("Recipient's private address"), recipientAddress);
    await ui.type(amounts[0] ?? document.body, "1");
    expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
  });

  it("does not let the user spend more than the synced private balance", async () => {
    const wallet = fakeWallet();
    const { user: ui } = await renderConnected(<WalletView />, wallet);
    await screen.findByText(/9\.3039215/);
    const amounts = screen.getAllByLabelText("Amount (USDC)");
    await ui.type(amounts[1] ?? document.body, "10");
    expect(screen.getByText("That is more than your private balance (9.3039215 USDC).")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Withdraw" })).toBeDisabled();
    await ui.clear(amounts[1] ?? document.body);
    await ui.type(amounts[1] ?? document.body, "9.3039215");
    expect(screen.getByRole("button", { name: "Withdraw" })).toBeEnabled();
  });

  it("explains a short private balance and lists the steps that were already sent", async () => {
    const failure = new PoolOperationError("failed", "transaction planning rejected: no combination possible", "insufficientPrivate", [HASH("ab")]);
    const { user: ui } = await renderConnected(<WalletView />, fakeWallet({ transfer: () => Promise.reject(failure) }));
    await screen.findByText(/9\.3039215/);
    await ui.type(screen.getByLabelText("Recipient's private address"), recipientAddress);
    await ui.type(screen.getAllByLabelText("Amount (USDC)")[0] ?? document.body, "1");
    await ui.click(screen.getByRole("button", { name: "Send" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Your private balance can't cover that amount.");
    expect(alert).not.toHaveTextContent("no combination possible");
    expect(alert).toHaveTextContent("These transactions were sent before it failed:");
    expect(alert.querySelector("a")).toHaveAttribute("href", expect.stringContaining(HASH("ab")));
  });

  it("shows the SDK's own text for a failure it can't explain", async () => {
    const { user: ui } = await renderConnected(<WalletView />, fakeWallet({ withdraw: () => Promise.reject(new Error("RPC timed out")) }));
    await screen.findByText(/9\.3039215/);
    await ui.type(screen.getAllByLabelText("Amount (USDC)")[1] ?? document.body, "1");
    await ui.click(screen.getByRole("button", { name: "Withdraw" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("RPC timed out");
  });

  it("reports a failed sync", async () => {
    await renderConnected(<WalletView />, fakeWallet({ balance: () => Promise.reject(new Error("RPC unavailable")) }));
    expect(await screen.findByRole("alert")).toHaveTextContent("RPC unavailable");
  });
});
