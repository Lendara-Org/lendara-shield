import type { AuditTransaction, PublicTransaction } from "@lendara/shield-api-types";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";

import { SessionProvider } from "@/components/session";
import { connectorFor, fakeWallet } from "@/test/fake-wallet";
import { HASH } from "@/test/fixtures";
import { apiError, server, url } from "@/test/server";

import { AuditView } from "./audit-view";

const TOKEN = "auditor-token-for-tests";
const INTAKE = "GCRN35RTRRZAIFFEMOTZFV7PNPMFOTDNXQKLMOQFSMOBU3LAON3A6NJC";
const CAROL = "GCWQUSCIB2S4LNW6FIOKL2N6BLFKAQ365A43IZVWJBR2ANAEDSIG3GQK";
const BOB = "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY";
const ISSUER = "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5";
const key = (b: string) => `0x${b.repeat(32)}`;

const txs: AuditTransaction[] = [
  {
    ledger: 5_392_310,
    tx_hash: HASH("2fe1"),
    kind: "transfer",
    ext_amount: "0",
    inputs: [{ owner_note_public_key: key("1d"), owner_stellar_account: INTAKE, amount: "9.3039215" }],
    outputs: [{ owner_note_public_key: key("c0"), owner_stellar_account: CAROL, amount: "9.3039215" }],
  },
  {
    ledger: 5_392_292,
    tx_hash: HASH("363a"),
    kind: "transfer",
    ext_amount: "0",
    inputs: [{ owner_note_public_key: key("c0"), owner_stellar_account: CAROL, amount: "2" }],
    outputs: [
      { owner_note_public_key: key("b0"), owner_stellar_account: BOB, amount: "0.5" },
      { owner_note_public_key: key("c0"), owner_stellar_account: CAROL, amount: "1.5" },
    ],
  },
];

const publicTx = (hash: string, source: string, visible: string[]): PublicTransaction => ({
  tx_hash: hash,
  ledger: 1,
  created_at: "2026-10-26T03:30:00Z",
  source_account: source,
  is_pool_transaction: true,
  ext_amount: "0",
  visible_accounts: visible,
  explorer_url: `https://stellar.expert/explorer/testnet/tx/${hash}`,
});

function renderAudit() {
  const user = userEvent.setup();
  render(
    <SessionProvider connectorFactory={connectorFor(fakeWallet())}>
      <AuditView />
    </SessionProvider>,
  );
  return user;
}

describe("AuditView", () => {
  it("shows the explorer's and the auditor's view of the same transaction", async () => {
    let authorization: string | null = null;
    server.use(
      http.get(url("/v1/audit/transactions"), ({ request }) => {
        authorization = request.headers.get("authorization");
        return HttpResponse.json({ transactions: txs });
      }),
      http.get(url(`/v1/audit/public/${HASH("2fe1")}`), () => HttpResponse.json(publicTx(HASH("2fe1"), INTAKE, [INTAKE]))),
      http.get(url(`/v1/audit/public/${HASH("363a")}`), () =>
        HttpResponse.json(publicTx(HASH("363a"), CAROL, [ISSUER, CAROL])),
      ),
    );
    const user = renderAudit();
    await user.type(screen.getByLabelText("Auditor token"), TOKEN);
    await user.click(screen.getByRole("button", { name: "Open the audit" }));

    const explorer = (await screen.findByRole("heading", { name: "What anyone sees on the explorer" })).parentElement;
    const auditor = screen.getByRole("heading", { name: "What the auditor sees" }).parentElement;
    if (!explorer || !auditor) throw new Error("missing panels");
    expect(authorization).toBe(`Bearer ${TOKEN}`);

    // The ramp's private transfer: the public sees only Shield's account.
    expect(await within(explorer).findByText("GCRN35…6NJC")).toBeInTheDocument();
    expect(within(explorer).getByText("Nothing: amount hidden")).toBeInTheDocument();
    expect(within(explorer).getByText("Not visible")).toBeInTheDocument();
    expect(within(explorer).queryByText(/GCWQUS/)).not.toBeInTheDocument();
    expect(within(auditor).getByText("GCWQUS…3GQK")).toBeInTheDocument();

    // A direct-mode transfer: the signer is visible; the auditor sees the recipient and the change.
    await user.click(screen.getByRole("button", { name: /363a/ }));
    expect(await within(explorer).findByText("GCWQUS…3GQK")).toBeInTheDocument();
    expect(within(explorer).getByText("GBVIQ7…47B5")).toBeInTheDocument();
    expect(within(auditor).getByText("GBJ3H7…PFWY")).toBeInTheDocument();
    expect(within(auditor).getByText(/\(change\)/)).toBeInTheDocument();
  });

  it("lists what each operation moved", async () => {
    server.use(
      http.get(url("/v1/audit/transactions"), () => HttpResponse.json({ transactions: txs })),
      http.get(url("/v1/audit/public/:hash"), ({ params }) =>
        HttpResponse.json(publicTx(String(params.hash), INTAKE, [INTAKE])),
      ),
    );
    const user = renderAudit();
    await user.type(screen.getByLabelText("Auditor token"), TOKEN);
    await user.click(screen.getByRole("button", { name: "Open the audit" }));
    const rows = await screen.findAllByRole("button", { name: /Private transfer/ });
    expect(rows.map((row) => row.textContent)).toEqual([expect.stringContaining("9.3039215 USDC"), expect.stringContaining("0.5 USDC")]);
  });

  it("rejects a wrong token without showing anything", async () => {
    server.use(http.get(url("/v1/audit/transactions"), () => apiError(401, "unauthorized", "Missing or invalid bearer token.")));
    const user = renderAudit();
    await user.type(screen.getByLabelText("Auditor token"), "wrong");
    await user.click(screen.getByRole("button", { name: "Open the audit" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That auditor token is not valid.");
    expect(screen.queryByText("What the auditor sees")).not.toBeInTheDocument();
  });

  it("explains when the auditor is not configured", async () => {
    server.use(http.get(url("/v1/audit/transactions"), () => apiError(503, "service_unavailable", "A dependency is temporarily unavailable.")));
    const user = renderAudit();
    await user.type(screen.getByLabelText("Auditor token"), TOKEN);
    await user.click(screen.getByRole("button", { name: "Open the audit" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("temporarily unavailable");
  });

  it("says when a transaction's hash is not known yet", async () => {
    server.use(
      http.get(url("/v1/audit/transactions"), () => HttpResponse.json({ transactions: [{ ...txs[0], tx_hash: null }] })),
    );
    const user = renderAudit();
    await user.type(screen.getByLabelText("Auditor token"), TOKEN);
    await user.click(screen.getByRole("button", { name: "Open the audit" }));
    expect(await screen.findByText(/hash could not be resolved/)).toBeInTheDocument();
  });
});
