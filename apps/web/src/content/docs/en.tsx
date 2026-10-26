import { A, C, Callout, CodeBlock, H2, H3, List, P, Table } from "@/components/docs/prose";
import { type DocChrome, type DocPage, type DocSlug, docHref } from "@/lib/docs";

import { CODE, REPO } from "./shared";

export const chrome: DocChrome = {
  docs: "Docs",
  previous: "Previous",
  next: "Next",
  menu: "Docs menu",
  copy: ["Copy", "Copied"],
};

const code = (value: string, title?: string) => <CodeBlock code={value} copy={chrome.copy} {...(title ? { title } : {})} />;

export const pages: Record<DocSlug, DocPage> = {
  "": {
    title: "Overview",
    description: "A compliant privacy layer that takes money from any SEP-24 anchor straight into a private pool.",
    body: () => (
      <>
        <Callout title="Testnet only" tone="warning">
          Shield runs on Stellar testnet over Stellar Private Payments, a developer preview with unaudited contracts.
          No real money moves.
        </Callout>

        <H2 id="problem">The problem</H2>
        <P>
          Stellar has privacy tools, but privacy breaks at the fiat edge. A ramp deposit pays the user&apos;s own
          address, with an amount and a time, and anyone can follow it into a privacy pool and out. A compliant pool
          also needs someone to run KYC, maintain the allow-list (the ASP), keep the auditor&apos;s view key and pay
          fees.
        </P>

        <H2 id="how">How Shield works</H2>
        <List ordered>
          <li>The user&apos;s wallet derives their pool keys. Shield receives only public keys, signed by the account.</li>
          <li>Shield runs KYC and, as the pool&apos;s ASP, adds the user to the allow-list on-chain.</li>
          <li>
            Shield opens a standard SEP-24 deposit with the anchor, paid to <strong>Shield&apos;s omnibus account</strong>{" "}
            with a memo.
          </li>
          <li>When the anchor confirms its payment, Shield deposits into the pool from its own account.</li>
          <li>Shield transfers the amount privately to the user&apos;s keys.</li>
        </List>
        <P>
          On the explorer you only see &quot;the anchor paid Shield&quot; and &quot;Shield used the pool&quot;. The
          user&apos;s address appears nowhere. The auditor, with the view key, sees who received what.
        </P>

        <H2 id="modes">Two modes</H2>
        <Table
          head={["", "Ramp", "Direct"]}
          rows={[
            ["Money comes from", "Local currency through a SEP-24 anchor", "USDC in the user's wallet"],
            ["User address on-chain", "Never", "When depositing, and as the signer of their transfers"],
            ["Needs an anchor", "Yes, any SEP-24 anchor", "No"],
          ]}
        />

        <H2 id="who">Who it is for</H2>
        <List>
          <li>
            <strong>Wallets and apps</strong> that want private balances for their users: integrate the API (see{" "}
            <A href={docHref("en", "quickstart")}>Quickstart</A>).
          </li>
          <li>
            <strong>Anchors</strong> that want to offer private deposits: they need no change for level 1, and can adopt
            the <A href={docHref("en", "protocol")}>Shield Protocol</A> natively.
          </li>
          <li>
            <strong>Auditors and regulators</strong>: every note is visible with the pool&apos;s view key (see{" "}
            <A href={docHref("en", "security")}>Security</A>).
          </li>
        </List>
      </>
    ),
  },

  protocol: {
    title: "Shield Protocol",
    description: "Level 1 works with any SEP-24 anchor today; level 2 lets anchors deposit into the pool natively.",
    body: () => (
      <>
        <P>
          The Shield Protocol is built on <A href="https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0024.md">SEP-24</A>,{" "}
          <A href="https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md">SEP-10</A>,{" "}
          <A href="https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0053.md">SEP-53</A> and a{" "}
          <A href="https://developers.stellar.org/docs/build/apps/privacy">Stellar Private Payments</A> pool with an
          allow-list, a block-list and a Global View Key. The name has no brand so any anchor can adopt it.
        </P>

        <H2 id="roles">Roles</H2>
        <Table
          head={["Role", "Does"]}
          rows={[
            ["User", "Derives pool keys in the wallet; spending keys never leave it"],
            ["Anchor", "SEP-24 on-ramp: KYC, takes the fiat payment, pays the asset"],
            ["ASP", "Maintains the pool's allow-list and block-list"],
            ["Auditor", "Holds the Global View Key and can decrypt every note"],
            ["Shield (level 1)", "ASP + omnibus account + SEP-24 client in front of any anchor"],
          ]}
        />

        <H2 id="level-1">Level 1: Shield as middleware (implemented)</H2>
        <H3>Onboarding</H3>
        <List ordered>
          <li>
            The wallet derives the note key, the encryption key and the ASP leaf. The user does <strong>not</strong>{" "}
            register in the pool&apos;s public key registry, because that transaction would publish their address.
          </li>
          <li>The account signs the keys (SEP-53) and sends them to the ASP.</li>
          <li>After KYC, the ASP inserts the leaf into the allow-list. Blocking inserts the note key into the block-list.</li>
        </List>
        <H3>Deposit</H3>
        <List ordered>
          <li>Shield authenticates with the anchor (SEP-10) as its omnibus account, with an <C>id</C> memo for the user.</li>
          <li>
            Shield requests an interactive deposit (SEP-24) to <strong>its omnibus account</strong>, with a random{" "}
            <C>id</C> memo for the intent.
          </li>
          <li>The user pays in the anchor&apos;s widget; the anchor pays the omnibus account.</li>
          <li>
            Shield funds the intent <strong>only with the transaction the anchor reports</strong> (
            <C>stellar_transaction_id</C>), never just because a payment carries the memo.
          </li>
          <li>Shield deposits into the pool (<C>ext_amount &gt; 0</C>), then transfers privately to the user (<C>ext_amount = 0</C>).</li>
        </List>
        <Callout title="Idempotency is part of the protocol">
          Every pool transaction is proved, signed and its hash stored <em>before</em> it is sent. A retry checks the
          stored hash and resends the same envelope. A replacement is only prepared when the original can no longer land.
        </Callout>

        <H2 id="level-2">Level 2: native shielded deposits (draft)</H2>
        <P>The anchor deposits straight into the pool to the user&apos;s keys, which removes Shield&apos;s custody step.</P>
        {code(`SHIELD_PROTOCOL_VERSION = "0.1"
SHIELD_POOL_CONTRACT = "C…"
SHIELD_ASP_ENDPOINT = "https://…"`, "stellar.toml")}
        {code(`{ "features": { "shielded_deposit": true } }`, "GET /info (SEP-24)")}
        {code(`POST /transactions/deposit/interactive
asset_code=USDC
destination_type=shielded
shielded_recipient=<note_public_key>:<encryption_public_key>`, "SEP-24 interactive deposit")}
        <P>
          The full draft, with settlement, withdrawals, security considerations and open questions, is in{" "}
          <A href={`${REPO}/docs/protocol/shield-protocol.md`}>docs/protocol/shield-protocol.md</A>.
        </P>
      </>
    ),
  },

  quickstart: {
    title: "Quickstart",
    description: "Give your users a private balance funded in local currency, with a few HTTP calls.",
    body: () => (
      <>
        <P>
          This is the flow Shield&apos;s own web follows. You need the{" "}
          <A href="https://www.npmjs.com/package/stellar-private-payments">stellar-private-payments</A> browser SDK
          (0.2.1) for the keys, and Freighter or any SEP-43 wallet for the signatures.
        </P>

        <H2 id="info">1. Read the deployment</H2>
        {code(`const info = await fetch(\`\${SHIELD}/v1/info\`).then((r) => r.json());
// info.network_passphrase, info.pool.contract_id, info.modes.ramp.enabled`)}

        <H2 id="keys">2. Derive the user&apos;s pool keys</H2>
        <P>The wallet signs once to derive them. They stay in the browser.</P>
        {code(CODE.keys)}

        <H2 id="onboard">3. Onboard with a signature</H2>
        <P>
          Sign this exact text with the account (SEP-53, Freighter&apos;s <C>signMessage</C>), with keys in lowercase
          hex. Without it, <C>400 invalid_signature</C>.
        </P>
        {code(CODE.onboard)}
        <P>
          Poll <C>GET /v1/users/{"{account}"}</C> while <C>asp_status</C> is <C>pending</C>, until it is{" "}
          <C>allowed</C>.
        </P>

        <H2 id="intent">4. Create a ramp intent</H2>
        {code(CODE.intent)}

        <H2 id="follow">5. Follow it to completed</H2>
        <P>Poll every 2–3 seconds. The three hashes appear as each step lands on-chain.</P>
        {code(CODE.follow)}

        <H2 id="balance">6. Show the private balance</H2>
        {code(CODE.balance)}
        <Callout title="Try it without code">
          The <A href="/en/onboard">Onboard</A>, <A href="/en/deposit">Deposit</A> and <A href="/en/wallet">Wallet</A>{" "}
          pages of this site run exactly this flow against testnet.
        </Callout>
      </>
    ),
  },

  api: {
    title: "API",
    description: "REST and JSON, defined in openapi.yaml. Testnet only.",
    body: () => (
      <>
        <P>
          The contract is <A href={`${REPO}/openapi.yaml`}>openapi.yaml</A>.{" "}
          <A href="/api-reference.html">Open the full API reference →</A>
        </P>

        <H2 id="endpoints">Endpoints</H2>
        <Table
          head={["Endpoint", "Auth", "What it does"]}
          rows={[
            [<C key="c">GET /v1/info</C>, "—", "Network, pool, asset and enabled modes"],
            [<C key="c">POST /v1/onboarding</C>, "SEP-53 signature", "Registers keys, runs the simulated KYC, requests the allow-list"],
            [<C key="c">GET /v1/users/{"{account}"}</C>, "—", "KYC and allow-list status"],
            [<C key="c">POST /v1/intents</C>, "SEP-53 signature", "Creates a ramp intent and returns the anchor's interactive URL (only here)"],
            [<C key="c">GET /v1/intents/{"{id}"}</C>, "—", "Status and on-chain hashes (never the interactive URL)"],
            [<C key="c">POST /v1/asp/block</C>, <C key="t">ADMIN_TOKEN</C>, "Blocks a key (block-list)"],
            [<C key="c">POST /v1/asp/unblock</C>, <C key="t">ADMIN_TOKEN</C>, "Unblocks a key"],
            [<C key="c">GET /v1/audit/transactions</C>, <C key="t">AUDITOR_TOKEN</C>, "Pool activity decrypted with the view key"],
            [<C key="c">GET /v1/audit/public/{"{tx_hash}"}</C>, "—", "What anyone sees about a transaction"],
          ]}
        />

        <H2 id="errors">Errors</H2>
        <P>Every error has a stable code:</P>
        {code(`{ "error": "user_not_allowed", "message": "…" }`)}
        <Table
          head={["Status", "Common codes"]}
          rows={[
            ["400", <><C>invalid_request</C>, <C>invalid_signature</C>, <C>invalid_key</C>, <C>invalid_limit</C></>],
            ["401", <C key="c">unauthorized</C>],
            ["404", <C key="c">not_found</C>],
            ["409", <><C>account_already_onboarded</C>, <C>pool_key_already_registered</C>, <C>user_not_allowed</C>, <C>ramp_mode_disabled</C></>],
            ["502 / 503", <><C>anchor_unavailable</C>, <C>service_unavailable</C></>],
          ]}
        />

        <H2 id="statuses">Intent statuses</H2>
        <Table
          head={["Status", "Meaning"]}
          rows={[
            [<C key="c">awaiting_anchor</C>, "The user has to pay at the anchor"],
            [<C key="c">funds_received</C>, "The anchor's payment is confirmed and in Shield's custody"],
            [<C key="c">shielding</C>, "The pool deposit is being sent"],
            [<C key="c">transferring</C>, "The private transfer is being sent"],
            [<C key="c">completed</C>, "The private balance is the user's"],
            [<><C>rejected</C>, <C>expired</C>, <C>error</C></>, "Final; see message"],
          ]}
        />
      </>
    ),
  },

  scenarios: {
    title: "Scenarios",
    description: "Force every outcome on testnet: Shield's KYC and the anchor's test cards.",
    body: () => (
      <>
        <H2 id="kyc">Shield&apos;s KYC (sandbox)</H2>
        <P>
          Choose it with <C>scenario</C> in <C>POST /v1/onboarding</C>, or in the selector on the Onboard page.
        </P>
        <Table
          head={["Scenario", "Result"]}
          rows={[
            [<C key="c">approved</C>, "The leaf joins the allow-list: the user can transact"],
            [<C key="c">rejected</C>, "Never inserted: the pool refuses the user's proofs and intents answer 409"],
            [<C key="c">blocked</C>, "Inserted, then block-listed: frozen until the ASP unblocks it"],
          ]}
        />

        <H2 id="cards">The anchor&apos;s test cards (Lendara sandbox)</H2>
        <P>Any future expiry date and any 3-digit CVC work.</P>
        <Table
          head={["Card", "Outcome", "Intent"]}
          rows={[
            [<C key="c">4242 4242 4242 4242</C>, "Approved", <C key="s">completed</C>],
            [<C key="c">4000 0000 0000 0002</C>, "KYC rejected", <C key="s">error</C>],
            [<C key="c">4000 0000 0000 9995</C>, "Payment not received", <C key="s">expired</C>],
            [<C key="c">4000 0000 0000 0077</C>, "Approved after 30 s", <C key="s">completed</C>],
          ]}
        />

        <H2 id="money">Money edge cases</H2>
        <List>
          <li>A payment with someone&apos;s memo that the anchor does not report never funds anything: it waits for review.</li>
          <li>Late or second payments are kept for review, never applied automatically.</li>
          <li>Timeouts, dropped transactions and RPC lag never pay twice: the stored hash is checked first.</li>
        </List>
      </>
    ),
  },

  security: {
    title: "Security",
    description: "What is private, from whom, and the limits we know about.",
    body: () => (
      <>
        <H2 id="who-sees">Who sees what (ramp mode)</H2>
        <Table
          head={["Observer", "Sees"]}
          rows={[
            ["The public", "The anchor paid Shield; Shield used the pool. Not the user's address, not who received what"],
            ["The anchor", "The user and the amount (it does the KYC)"],
            ["Shield", "Which account owns which keys (it is the ASP), and each intent"],
            ["The auditor", "Every note: owner and amount"],
          ]}
        />

        <H2 id="protections">Protections</H2>
        <List>
          <li>Spending keys never leave the user&apos;s browser.</li>
          <li>Onboarding needs the account&apos;s SEP-53 signature over the exact keys.</li>
          <li>Starting a deposit needs the account&apos;s SEP-53 signature over the amount, and each signed request works once.</li>
          <li>Only the payment the anchor reports funds an intent; intent memos are random.</li>
          <li>The view key only reaches the auditor&apos;s process, never a browser.</li>
          <li>Operator, omnibus and auditor are three different keys.</li>
          <li>Logs never pair a user&apos;s address with an amount.</li>
        </List>

        <H2 id="limits">Known limits</H2>
        <List>
          <li>The anchor&apos;s payment and Shield&apos;s deposit share the amount and the time, so they can be linked (batching is next).</li>
          <li>In direct mode, the user signs their own transfers, so their account shows as a pool user (a relayer fixes it).</li>
          <li><C>GET /v1/users/{"{account}"}</C> is public: it tells whether an account uses Shield.</li>
          <li>The asset issuer could enable clawback later; it is not <C>AUTH_IMMUTABLE</C>.</li>
          <li>Unaudited developer-preview contracts; the public RPC keeps about 7 days of history.</li>
        </List>
        <P>
          Full threat model: <A href={`${REPO}/docs/threat-model.md`}>docs/threat-model.md</A>.
        </P>
      </>
    ),
  },
};
