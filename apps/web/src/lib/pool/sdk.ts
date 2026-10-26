/**
 * The real `WalletConnector`: Freighter + the Stellar Private Payments browser
 * SDK. The SDK (WASM, storage and prover workers, circuits) is served from
 * `/spp` by `scripts/stage-web-sdk.sh` and imported at runtime, so its
 * workers keep resolving their files relative to themselves.
 */
import type * as Spp from "stellar-private-payments";
// Pinned at build time: artifacts fetched at runtime are checked against it.
import circuitLock from "stellar-private-payments/circuits/circuits.json";

import { SDK_BASE_PATH, SOROBAN_RPC_URL } from "../config";
import { circuitsBaseUrl, preloadPoolCircuits } from "./circuits";
import { FreighterSigner, connectFreighter } from "./freighter-signer";
import { withOneProverRetry } from "./retry";
import { PoolOperationError, type PoolResult, type RecipientKeys, type ShieldWallet, type WalletConnector } from "./types";

type SppModule = typeof Spp;

let runtime: Promise<{ sdk: SppModule; client: Spp.Client; poolContract: string }> | null = null;

async function loadRuntime() {
  const origin = window.location.origin;
  const sdk = (await import(
    /* webpackIgnore: true */ /* turbopackIgnore: true */ `${origin}${SDK_BASE_PATH}/sdk/js/index.js`
  )) as SppModule;
  await sdk.default();
  const response = await fetch(`${SDK_BASE_PATH}/deployments.json`);
  if (!response.ok) throw new Error("Pool deployment not found: run `make stage-web-sdk`.");
  const contractConfig = (await response.json()) as Spp.ContractConfigInput;
  const poolContract = contractConfig.pools[0]?.poolContractId;
  if (!poolContract) throw new Error("The deployment has no pool.");

  const storage = await sdk.Storage.open();
  if (await sdk.bootnodeRequired(SOROBAN_RPC_URL, storage, { contractConfig })) {
    throw new Error("The pool is older than the RPC's history window: a bootnode is required.");
  }
  const client = await sdk.Client.new({
    rpcUrl: SOROBAN_RPC_URL,
    storage,
    contractConfig,
    circuitsBaseUrl: circuitsBaseUrl(origin),
    circuitLock,
  });
  return { sdk, client, poolContract };
}

/** One SDK runtime per page load; a failed load can be retried. */
function sppRuntime() {
  runtime ??= loadRuntime().catch((err: unknown) => {
    runtime = null;
    throw err;
  });
  return runtime;
}

function done(result: Spp.PoolExecuteResult): PoolResult {
  if (result.status !== "ok") {
    const message =
      result.status === "aspNotReady"
        ? "The pool has not seen your allow-list entry yet. Try again in a minute."
        : (result.message ?? "The pool rejected the operation.");
    throw new PoolOperationError(result.status, message);
  }
  return { hashes: [...result.hashes] };
}

export function sppConnector(networkPassphrase: string): WalletConnector {
  return {
    async connect(): Promise<ShieldWallet> {
      const address = await connectFreighter(networkPassphrase);
      const { client, poolContract } = await sppRuntime();
      const signer = new FreighterSigner();
      const account = await client.account({ networkPassphrase, userAddress: address }, signer);
      // The SDK only finds this user's notes once their keys are derived in
      // this browser's storage (idempotent: Freighter signs once per browser).
      let keys: Promise<Spp.UserPublicKeys> | null = null;
      const deriveKeys = () =>
        (keys ??= account.derivePrivacyKeys().catch((err: unknown) => {
          keys = null;
          throw err;
        }));
      let pool: Promise<Spp.PrivatePool> | null = null;
      const openPool = async () => {
        await deriveKeys();
        return (pool ??= account.pool({ poolContract }));
      };

      return {
        address,
        async keys() {
          const derived = await deriveKeys();
          return {
            notePublicKey: derived.notePublicKey,
            encryptionPublicKey: derived.encryptionPublicKey,
            aspLeaf: await account.deriveAspUserLeaf(),
          };
        },
        async signMessage(message: string) {
          const { signedMessage } = await signer.signMessage(message, { address, networkPassphrase });
          return signedMessage;
        },
        async balance() {
          const opened = await openPool();
          await client.sync();
          return opened.balance();
        },
        // Each operation proves: the circuit is downloaded first, outside
        // the prover's 30 s budget, and a proving timeout is retried once.
        async deposit(amount: bigint) {
          const [opened] = await Promise.all([openPool(), preloadPoolCircuits()]);
          return done(await withOneProverRetry(() => opened.deposit(amount)));
        },
        async transfer(to: RecipientKeys, amount: bigint) {
          const [opened] = await Promise.all([openPool(), preloadPoolCircuits()]);
          await client.sync();
          return done(
            await withOneProverRetry(() => opened.transferToKeys(to.notePublicKey, to.encryptionPublicKey, amount)),
          );
        },
        async withdraw(amount: bigint) {
          const [opened] = await Promise.all([openPool(), preloadPoolCircuits()]);
          await client.sync();
          return done(await withOneProverRetry(() => opened.withdraw(amount)));
        },
      };
    },
  };
}
