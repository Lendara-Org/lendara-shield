/**
 * Whether a Stellar account holds a trustline to the pool's asset.
 *
 * A direct deposit spends the account's USDC and a withdraw pays USDC back
 * to it: both fail without a trustline, but only once the SDK simulates the
 * pool transaction, after proving. Reading the trustline first lets the
 * pages say what is missing. It asks the Stellar RPC (`getLedgerEntries`)
 * because the CSP only allows the API and the RPC, and builds the ledger key
 * here so the web needs no Stellar SDK.
 */

import { SOROBAN_RPC_URL } from "./config";

export interface ClassicAsset {
  code: string;
  issuer: string;
}

/** `present`/`missing` come from the ledger; `unknown` means the RPC could not tell. */
export type TrustlineState = "present" | "missing" | "unknown";

const RPC_TIMEOUT_MS = 10_000;
const BASE32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
/** Strkey version byte of an ed25519 public key (`G…`). */
const ACCOUNT_VERSION = 6 << 3;
const LEDGER_KEY_TRUSTLINE = 1;
const ASSET_CREDIT_ALPHANUM4 = 1;
const ASSET_CREDIT_ALPHANUM12 = 2;

/** CRC16-XModem, the checksum of Stellar strkeys. */
function crc16(bytes: Uint8Array): number {
  let crc = 0;
  for (const byte of bytes) {
    crc ^= byte << 8;
    for (let bit = 0; bit < 8; bit++) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  return crc;
}

/** The 32-byte ed25519 key of a `G…` account, or `null` if it is not one. */
export function accountKey(account: string): Uint8Array | null {
  if (!/^G[A-Z2-7]{55}$/.test(account)) return null;
  const bytes = new Uint8Array(35);
  let buffer = 0;
  let bits = 0;
  let index = 0;
  for (const char of account) {
    buffer = (buffer << 5) | BASE32.indexOf(char);
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      bytes[index++] = (buffer >> bits) & 0xff;
    }
  }
  const checksum = (bytes[33] ?? 0) | ((bytes[34] ?? 0) << 8);
  if (bytes[0] !== ACCOUNT_VERSION || crc16(bytes.subarray(0, 33)) !== checksum) return null;
  return bytes.slice(1, 33);
}

function int32(value: number): number[] {
  return [(value >>> 24) & 0xff, (value >>> 16) & 0xff, (value >>> 8) & 0xff, value & 0xff];
}

/** Base64 XDR of the `LedgerKey` of `account`'s trustline to `asset`, or `null` if either is invalid. */
export function trustlineKey(account: string, asset: ClassicAsset): string | null {
  const owner = accountKey(account);
  const issuer = accountKey(asset.issuer);
  if (!owner || !issuer || !/^[A-Za-z0-9]{1,12}$/.test(asset.code)) return null;
  const width = asset.code.length <= 4 ? 4 : 12;
  const code = new Array<number>(width).fill(0);
  for (let i = 0; i < asset.code.length; i++) code[i] = asset.code.charCodeAt(i);
  const bytes = [
    ...int32(LEDGER_KEY_TRUSTLINE),
    ...int32(0), // PUBLIC_KEY_TYPE_ED25519
    ...owner,
    ...int32(width === 4 ? ASSET_CREDIT_ALPHANUM4 : ASSET_CREDIT_ALPHANUM12),
    ...code,
    ...int32(0),
    ...issuer,
  ];
  return btoa(String.fromCharCode(...bytes));
}

/** Reads the trustline from the RPC. Never throws: a failed read is `unknown`. */
export async function readTrustline(account: string, asset: ClassicAsset): Promise<TrustlineState> {
  const key = trustlineKey(account, asset);
  if (!key) return "unknown";
  try {
    const response = await fetch(SOROBAN_RPC_URL, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getLedgerEntries", params: { keys: [key] } }),
      signal: AbortSignal.timeout(RPC_TIMEOUT_MS),
    });
    if (!response.ok) return "unknown";
    const body: unknown = await response.json();
    const result = typeof body === "object" && body !== null ? (body as Record<string, unknown>).result : undefined;
    if (typeof result !== "object" || result === null) return "unknown";
    const entries = (result as Record<string, unknown>).entries;
    // The RPC omits `entries` (or sends `null`) when none of the keys exist.
    if (entries === undefined || entries === null) return "missing";
    if (!Array.isArray(entries)) return "unknown";
    return entries.length > 0 ? "present" : "missing";
  } catch {
    return "unknown";
  }
}
