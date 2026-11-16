import type { NextConfig } from "next";

import { API_REFERENCE_CSP, contentSecurityPolicy } from "./src/lib/csp";
import { DOCS_URL } from "./src/lib/links";

// Same defaults as `src/lib/config.ts`.
const csp = contentSecurityPolicy({
  apiBaseUrl: process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8080",
  sorobanRpcUrl: process.env.NEXT_PUBLIC_SOROBAN_RPC_URL ?? "https://soroban-testnet.stellar.org",
  bootnodeUrl: process.env.NEXT_PUBLIC_BOOTNODE_URL?.trim() || undefined,
  development: process.env.NODE_ENV === "development",
});

const nextConfig: NextConfig = {
  poweredByHeader: false,
  // Pages have one root layout per language (app/[lang]/layout.tsx), so an
  // unmatched URL needs app/global-not-found.tsx (experimental in Next 16).
  experimental: { globalNotFound: true },
  // The API types package ships TypeScript sources.
  transpilePackages: ["@lendara/shield-api-types"],
  async redirects() {
    // Pages moved under /<lang>; old links keep working in English.
    const pages = ["onboard", "deposit", "wallet", "audit"].map((page) => ({
      source: `/${page}/:rest*`,
      destination: `/en/${page}/:rest*`,
      permanent: false,
    }));
    // The docs moved to GitBook; every old docs link lands there.
    const docs = ["/docs/:rest*", "/:lang(en|es)/docs/:rest*"].map((source) => ({
      source,
      destination: DOCS_URL,
      permanent: false,
    }));
    return [...pages, ...docs];
  },
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          { key: "Content-Security-Policy", value: csp },
          { key: "X-Frame-Options", value: "DENY" },
        ],
      },
      {
        // The generated API reference loads Redoc from its CDN (last match wins).
        source: "/api-reference.html",
        headers: [{ key: "Content-Security-Policy", value: API_REFERENCE_CSP }],
      },
      {
        // Circuit artifacts and the SDK build never change for a deployment.
        source: "/spp/sdk/:path*",
        headers: [{ key: "Cache-Control", value: "public, max-age=86400" }],
      },
    ];
  },
};

export default nextConfig;
