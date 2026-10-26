import { API_REFERENCE_CSP, contentSecurityPolicy } from "./csp";

const directives = (policy: string) =>
  Object.fromEntries(
    policy.split("; ").map((d) => {
      const [name = "", ...values] = d.split(" ");
      return [name, values];
    }),
  );

describe("contentSecurityPolicy", () => {
  const production = directives(
    contentSecurityPolicy({
      apiBaseUrl: "https://api.shield.example/",
      sorobanRpcUrl: "https://soroban-testnet.stellar.org",
      development: false,
    }),
  );

  it("sends data only to the app, Shield's API and the RPC", () => {
    expect(production["connect-src"]).toEqual(["'self'", "https://api.shield.example", "https://soroban-testnet.stellar.org"]);
  });

  it("runs only the app's own code and the SDK's WebAssembly", () => {
    expect(production["script-src"]).toEqual(["'self'", "'unsafe-inline'", "'wasm-unsafe-eval'"]);
    expect(production["script-src"]).not.toContain("'unsafe-eval'");
    expect(production["worker-src"]).toEqual(["'self'"]);
  });

  it("cannot be framed, embed plugins or post forms elsewhere", () => {
    expect(production["frame-ancestors"]).toEqual(["'none'"]);
    expect(production["frame-src"]).toEqual(["'none'"]);
    expect(production["object-src"]).toEqual(["'none'"]);
    expect(production["base-uri"]).toEqual(["'self'"]);
    expect(production["form-action"]).toEqual(["'self'"]);
  });

  it("allows Fast Refresh in development only", () => {
    const dev = directives(
      contentSecurityPolicy({ apiBaseUrl: "http://localhost:8080", sorobanRpcUrl: "http://localhost:8000", development: true }),
    );
    expect(dev["script-src"]).toContain("'unsafe-eval'");
    expect(dev["connect-src"]).toEqual(["'self'", "http://localhost:8080", "http://localhost:8000", "ws:"]);
  });
});

describe("API_REFERENCE_CSP", () => {
  it("adds only Redocly's CDN for scripts", () => {
    expect(directives(API_REFERENCE_CSP)["script-src"]).toEqual(["'self'", "'unsafe-inline'", "https://cdn.redocly.com"]);
    expect(directives(API_REFERENCE_CSP)["frame-ancestors"]).toEqual(["'none'"]);
  });
});
