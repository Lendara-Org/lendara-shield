import { HttpResponse, http } from "msw";

import { SOROBAN_RPC_URL } from "@/lib/config";
import { server } from "@/test/server";

import { accountKey, readTrustline, trustlineKey } from "./trustline";

const ISSUER = "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5";
const OWNER = "GDMSOZPN7R4HKQOTDKA52HSB4AAMNNPJIEOFPMN43VPYZN6GQOJ3KOW7";
const USDC = { code: "USDC", issuer: ISSUER };
// Encoded with `stellar xdr encode --type LedgerKey` (stellar-cli 28.1.0).
const USDC_KEY =
  "AAAAAQAAAADZJ2Xt/Hh1QdMagd0eQeAAxrXpQRxXsbzdX4y3xoOTtQAAAAFVU0RDAAAAAGqIfhWc08ass8YK8irYTYDFDA4uJR7rJrJLI05Vc7p+";
const LONG_KEY =
  "AAAAAQAAAADZJ2Xt/Hh1QdMagd0eQeAAxrXpQRxXsbzdX4y3xoOTtQAAAAJMT05HQVNTRVQAAAAAAAAAaoh+FZzTxqyzxgryKthNgMUMDi4lHusmsksjTlVzun4=";

function rpc(answer: () => Response) {
  const keys: unknown[] = [];
  server.use(
    http.post(SOROBAN_RPC_URL, async ({ request }) => {
      const body = (await request.json()) as { method: string; params: { keys: unknown[] } };
      expect(body.method).toBe("getLedgerEntries");
      keys.push(...body.params.keys);
      return answer();
    }),
  );
  return keys;
}

describe("trustlineKey", () => {
  it("matches the Stellar CLI for 4- and 12-character asset codes", () => {
    expect(trustlineKey(OWNER, USDC)).toBe(USDC_KEY);
    expect(trustlineKey(OWNER, { code: "LONGASSET", issuer: ISSUER })).toBe(LONG_KEY);
  });

  it("rejects bad accounts, issuers and codes", () => {
    expect(trustlineKey("not-an-account", USDC)).toBeNull();
    expect(trustlineKey(OWNER, { code: "USDC", issuer: "SBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5" })).toBeNull();
    expect(trustlineKey(OWNER, { code: "", issuer: ISSUER })).toBeNull();
    expect(trustlineKey(OWNER, { code: "THIRTEENCHARS", issuer: ISSUER })).toBeNull();
  });
});

describe("accountKey", () => {
  it("rejects a strkey whose checksum is wrong", () => {
    expect(accountKey(OWNER)).toHaveLength(32);
    expect(accountKey(`${OWNER.slice(0, -1)}A`)).toBeNull();
  });
});

describe("readTrustline", () => {
  it("is present when the RPC returns the entry", async () => {
    const keys = rpc(() => HttpResponse.json({ jsonrpc: "2.0", id: 1, result: { entries: [{ key: USDC_KEY, xdr: "AAAA" }], latestLedger: 5 } }));
    expect(await readTrustline(OWNER, USDC)).toBe("present");
    expect(keys).toEqual([USDC_KEY]);
  });

  it("is missing when the RPC returns no entries, empty or omitted", async () => {
    rpc(() => HttpResponse.json({ jsonrpc: "2.0", id: 1, result: { entries: [], latestLedger: 5 } }));
    expect(await readTrustline(OWNER, USDC)).toBe("missing");
    rpc(() => HttpResponse.json({ jsonrpc: "2.0", id: 1, result: { latestLedger: 5 } }));
    expect(await readTrustline(OWNER, USDC)).toBe("missing");
  });

  it("is unknown when the RPC fails, errors or answers garbage", async () => {
    rpc(() => new HttpResponse(null, { status: 503 }));
    expect(await readTrustline(OWNER, USDC)).toBe("unknown");
    rpc(() => HttpResponse.json({ jsonrpc: "2.0", id: 1, error: { code: -32600, message: "bad" } }));
    expect(await readTrustline(OWNER, USDC)).toBe("unknown");
    rpc(() => HttpResponse.json({ jsonrpc: "2.0", id: 1, result: { entries: "nope" } }));
    expect(await readTrustline(OWNER, USDC)).toBe("unknown");
    rpc(() => HttpResponse.error());
    expect(await readTrustline(OWNER, USDC)).toBe("unknown");
  });

  it("does not call the RPC for an invalid account", async () => {
    const keys = rpc(() => HttpResponse.json({}));
    expect(await readTrustline("GBAD", USDC)).toBe("unknown");
    expect(keys).toEqual([]);
  });
});
