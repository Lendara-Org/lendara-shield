/**
 * Content-Security-Policy of the web app (set in `next.config.ts`).
 *
 * The app loads its code, the SPP SDK, its WASM, workers and circuits from
 * its own origin, and talks only to Shield's API and the Stellar RPC. The
 * policy keeps it that way: no other script, frame, plugin or form target,
 * and data can only be sent to those two origins.
 *
 * Limits: `script-src` needs `'unsafe-inline'` because Next inlines its
 * hydration scripts in statically rendered pages (no per-request nonce), and
 * `'wasm-unsafe-eval'` because the SDK compiles WebAssembly. Development adds
 * `'unsafe-eval'` and WebSockets for Fast Refresh.
 */

export interface CspOptions {
  apiBaseUrl: string;
  sorobanRpcUrl: string;
  development: boolean;
}

/** Origin of a configured URL (`https://host[:port]`). */
function origin(url: string): string {
  return new URL(url).origin;
}

export function contentSecurityPolicy({ apiBaseUrl, sorobanRpcUrl, development }: CspOptions): string {
  const script = ["'self'", "'unsafe-inline'", "'wasm-unsafe-eval'", ...(development ? ["'unsafe-eval'"] : [])];
  const connect = ["'self'", origin(apiBaseUrl), origin(sorobanRpcUrl), ...(development ? ["ws:"] : [])];
  return [
    "default-src 'self'",
    `script-src ${script.join(" ")}`,
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data:",
    "font-src 'self'",
    `connect-src ${[...new Set(connect)].join(" ")}`,
    "worker-src 'self'",
    "frame-src 'none'",
    "frame-ancestors 'none'",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'self'",
  ].join("; ");
}

/**
 * Policy of `/api-reference.html`, the static page Redocly generates from
 * `openapi.yaml`: it loads Redoc from Redocly's CDN (pinned with SRI) and
 * never touches a wallet.
 */
export const API_REFERENCE_CSP = [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' https://cdn.redocly.com",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: https:",
  "font-src 'self' data:",
  "connect-src 'self'",
  "worker-src 'self' blob:",
  "frame-ancestors 'none'",
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'none'",
].join("; ");
