import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";

import { API_BASE_URL, SOROBAN_RPC_URL } from "@/lib/config";

import { info } from "./fixtures";

export const url = (path: string) => `${API_BASE_URL}${path}`;

/** The RPC's `getLedgerEntries` answer for a trustline that exists or not. */
export const rpcTrustline = (exists: boolean) =>
  http.post(SOROBAN_RPC_URL, () =>
    HttpResponse.json({ jsonrpc: "2.0", id: 1, result: { entries: exists ? [{ key: "k", xdr: "x" }] : [], latestLedger: 1 } }),
  );

/**
 * By default the API answers `/v1/info` and the RPC says the account trusts
 * the pool's asset; each test adds what it needs.
 */
export const server = setupServer(http.get(url("/v1/info"), () => HttpResponse.json(info)), rpcTrustline(true));

export const apiError = (status: number, error: string, message: string) =>
  HttpResponse.json({ error, message }, { status });
