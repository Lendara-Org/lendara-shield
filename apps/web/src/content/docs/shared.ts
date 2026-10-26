/** Shared by both languages: repository links and code samples. */

export const REPO = "https://github.com/Lendara-Org/lendara-shield/blob/develop";

export const CODE = {
  keys: `import init, { Client, Storage } from "stellar-private-payments";

await init();
const client = await Client.new({ rpcUrl, storage: await Storage.open(), contractConfig, circuitsBaseUrl, circuitLock });
const account = await client.account({ networkPassphrase, userAddress }, signer); // e.g. Freighter
const keys = await account.derivePrivacyKeys(); // the wallet signs once
const aspLeaf = await account.deriveAspUserLeaf();`,

  onboard: `const message = [
  "Lendara Shield onboarding",
  "network: testnet",
  \`account: \${userAddress}\`,
  \`note_public_key: \${keys.notePublicKey.toLowerCase()}\`,
  \`encryption_public_key: \${keys.encryptionPublicKey.toLowerCase()}\`,
  \`asp_leaf: \${aspLeaf.toLowerCase()}\`,
].join("\\n");
const { signedMessage } = await signMessage(message, { address: userAddress, networkPassphrase });

await fetch(\`\${SHIELD}/v1/onboarding\`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({
    stellar_account: userAddress,
    note_public_key: keys.notePublicKey.toLowerCase(),
    encryption_public_key: keys.encryptionPublicKey.toLowerCase(),
    asp_leaf: aspLeaf.toLowerCase(),
    signature: signedMessage,
    scenario: "approved", // sandbox only
  }),
});`,

  intent: `const intent = await fetch(\`\${SHIELD}/v1/intents\`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ mode: "ramp", stellar_account: userAddress, fiat_currency: "CRC", amount: "5000" }),
}).then((r) => r.json());

window.open(intent.interactive_url, "anchor", "popup"); // the anchor's SEP-24 widget`,

  follow: `const FINAL = ["completed", "rejected", "expired", "error"];
let current = intent;
while (!FINAL.includes(current.status)) {
  await new Promise((r) => setTimeout(r, 2500));
  current = await fetch(\`\${SHIELD}/v1/intents/\${intent.id}\`).then((r) => r.json());
}
// current.transactions: { intake_payment, pool_deposit, pool_transfer }`,

  balance: `const pool = await account.pool({ poolContract: info.pool.contract_id });
await client.sync();
const stroops = await pool.balance(); // bigint, 7 decimals`,
};
