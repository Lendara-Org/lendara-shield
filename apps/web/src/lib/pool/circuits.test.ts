import { SDK_CIRCUIT_CACHE, circuitUrls, circuitsBaseUrl, preloadCircuits } from "./circuits";

/** A fresh module: the preload is memoised per page. */
async function freshPreload() {
  vi.resetModules();
  return (await import("./circuits")).preloadPoolCircuits;
}

/** In-memory Cache Storage: records what was fetched into it. */
function fakeStorage(cached: string[] = []) {
  const entries = new Set(cached);
  const added: string[] = [];
  const opened: string[] = [];
  const cache = {
    match: vi.fn(async (url: string) => (entries.has(url) ? new Response("x") : undefined)),
    add: vi.fn(async (url: string) => {
      entries.add(url);
      added.push(url);
    }),
  };
  const storage = {
    open: vi.fn(async (name: string) => {
      opened.push(name);
      return cache;
    }),
  } as unknown as CacheStorage;
  return { storage, added, opened };
}

describe("circuitUrls", () => {
  it("names the files the SDK asks for (its artifact file names)", () => {
    expect(circuitUrls(circuitsBaseUrl("https://shield.example"), ["AB_gvk_T"])).toEqual([
      "https://shield.example/spp/sdk/dist/circuits/policy_tx_2_2_AB_gvk_T.graph.bin",
      "https://shield.example/spp/sdk/dist/circuits/policy_tx_2_2_AB_gvk_T.r1cs",
      "https://shield.example/spp/sdk/dist/circuits/policy_tx_2_2_AB_gvk_T_proving_key.bin",
    ]);
  });
});

describe("preloadCircuits", () => {
  const urls = circuitUrls("https://s/c/", ["AB_gvk_T"]);

  it("fills the SDK's own cache with the missing files only", async () => {
    const { storage, added, opened } = fakeStorage([urls[0]!]);
    await expect(preloadCircuits(urls, storage)).resolves.toBe(2);
    expect(opened).toEqual([SDK_CIRCUIT_CACHE]);
    expect(added).toEqual(urls.slice(1));
    await expect(preloadCircuits(urls, storage)).resolves.toBe(0);
  });

  it("does nothing without Cache Storage", async () => {
    await expect(preloadCircuits(urls, undefined)).resolves.toBe(0);
  });
});

describe("preloadPoolCircuits", () => {
  const original = globalThis.caches;
  afterEach(() => {
    vi.unstubAllGlobals();
    Object.defineProperty(globalThis, "caches", { value: original, configurable: true });
  });

  it("preloads the deployment's verifier circuits", async () => {
    const { storage, added } = fakeStorage();
    Object.defineProperty(globalThis, "caches", { value: storage, configurable: true });
    vi.stubGlobal("fetch", vi.fn(async () => Response.json({ verifiers: { AB_gvk_T: {} } })));
    const preloadPoolCircuits = await freshPreload();
    await preloadPoolCircuits();
    await preloadPoolCircuits();
    // Once per page: the second call reuses the first.
    expect(added).toEqual(circuitUrls(circuitsBaseUrl(window.location.origin), ["AB_gvk_T"]));
  });

  it("never rejects, and tries again after a failure", async () => {
    const fetch = vi.fn(async () => new Response("", { status: 503 }));
    vi.stubGlobal("fetch", fetch);
    const preloadPoolCircuits = await freshPreload();
    await expect(preloadPoolCircuits()).resolves.toBeUndefined();
    await expect(preloadPoolCircuits()).resolves.toBeUndefined();
    expect(fetch).toHaveBeenCalledTimes(2);
  });
});
