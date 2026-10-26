import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";

import { connectorFor, fakeWallet } from "@/test/fake-wallet";
import { ACCOUNT, user } from "@/test/fixtures";
import { renderConnected } from "@/test/render";
import { server, url } from "@/test/server";

import { ConnectGate } from "./connect-gate";
import { SessionProvider } from "./session";

const content = () => <p>private content</p>;

describe("ConnectGate", () => {
  it("explains why the wallet could not connect", async () => {
    render(
      <SessionProvider connectorFactory={connectorFor(new Error("Switch Freighter to Testnet and try again."))}>
        <ConnectGate>{content}</ConnectGate>
      </SessionProvider>,
    );
    const button = await screen.findByRole("button", { name: "Connect Freighter" });
    await vi.waitFor(() => expect(button).toBeEnabled());
    await userEvent.click(button);
    expect(await screen.findByRole("alert")).toHaveTextContent("Switch Freighter to Testnet");
    expect(screen.queryByText("private content")).not.toBeInTheDocument();
  });

  it("says when the API is down", async () => {
    server.use(http.get(url("/v1/info"), () => HttpResponse.error()));
    render(
      <SessionProvider connectorFactory={connectorFor(fakeWallet())}>
        <ConnectGate>{content}</ConnectGate>
      </SessionProvider>,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("unreachable");
  });

  it("sends users who are not allowed to onboarding", async () => {
    server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => HttpResponse.json(user({ asp_status: "blocked" }))));
    await renderConnected(<ConnectGate requireAllowed>{content}</ConnectGate>, fakeWallet());
    expect(await screen.findByText(/Your key is blocked/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Go to onboarding" })).toHaveAttribute("href", "/onboard");
    expect(screen.queryByText("private content")).not.toBeInTheDocument();
  });

  it("renders the content for an allowed user", async () => {
    server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => HttpResponse.json(user())));
    await renderConnected(<ConnectGate requireAllowed>{content}</ConnectGate>, fakeWallet());
    expect(await screen.findByText("private content")).toBeInTheDocument();
  });
});
