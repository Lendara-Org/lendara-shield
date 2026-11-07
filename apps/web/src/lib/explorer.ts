/** stellar.expert links (testnet only). */

const BASE = "https://stellar.expert/explorer/testnet";

export function txUrl(hash: string): string {
  return `${BASE}/tx/${hash}`;
}

export function accountUrl(account: string): string {
  return `${BASE}/account/${account}`;
}

/** `d43d5727…53a7`, for display; the full value goes in links and titles. */
export function shorten(value: string, head = 8, tail = 4): string {
  return value.length <= head + tail + 1 ? value : `${value.slice(0, head)}…${value.slice(-tail)}`;
}
