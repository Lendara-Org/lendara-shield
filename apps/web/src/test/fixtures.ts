/** API payloads shaped like the real testnet ones (public data only). */
import type { Info, Intent, ShieldUser } from "@lendara/shield-api-types";

export const ACCOUNT = "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY";
export const NOTE_KEY = `0x${"a2".repeat(32)}`;
export const ENC_KEY = `0x${"01".repeat(32)}`;
export const LEAF = `0x${"05".repeat(32)}`;
export const HASH = (b: string) => b.repeat(64).slice(0, 64);

export const info: Info = {
  network: "testnet",
  network_passphrase: "Test SDF Network ; September 2015",
  pool: {
    contract_id: "CCUJRW6VOQBFT5EWSKTYF24WHLFQFTO6SH33VFWAGOJFFSYGXPWXK4HF",
    asset: {
      code: "USDC",
      issuer: "GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5",
      contract_id: "CAE6WLIMEMDSRSZZW7H3O7DJS2DRDV5NTR2FHE53TKGJMJOHWGKSR4QX",
    },
    asp_policy: ["allowlist", "blocklist"],
    asp_membership_contract: "CDUJHXHC2DXXOXKBIK5E5QDKD6C3WUSMDIUFF6QTCECBNI4S2656VYD6",
    asp_non_membership_contract: "CA46MJAIJTYJ5CO4OLRYRRTRJ4MVGO55LB6RQA6756FUNZPU2UTRD463",
    gvk_mode: "traceable",
    kdf_domain: "Lendara Shield",
  },
  modes: { ramp: { enabled: true, anchor_home_domain: "lendara-monorepo-fixh.vercel.app" }, direct: { enabled: true } },
};

export function user(overrides: Partial<ShieldUser> = {}): ShieldUser {
  return {
    stellar_account: ACCOUNT,
    kyc_status: "approved",
    asp_status: "allowed",
    last_asp_tx_hash: HASH("17c2"),
    created_at: "2026-10-26T02:50:00Z",
    updated_at: "2026-10-26T02:50:05Z",
    ...overrides,
  };
}

export function intent(overrides: Partial<Intent> = {}): Intent {
  return {
    id: "01a1761b-e7ac-7557-9615-12e3e773265c",
    mode: "ramp",
    status: "awaiting_anchor",
    fiat_currency: "CRC",
    amount_fiat: "5000",
    asset_code: "USDC",
    amount_asset: null,
    interactive_url: "https://anchor.example/widget/tx?token=t",
    transactions: { intake_payment: null, pool_deposit: null, pool_transfer: null },
    message: null,
    created_at: "2026-10-26T02:51:00Z",
    updated_at: "2026-10-26T02:51:00Z",
    completed_at: null,
    ...overrides,
  };
}

export const completed = intent({
  status: "completed",
  amount_asset: "9.3039215",
  interactive_url: null,
  transactions: { intake_payment: HASH("2fc5"), pool_deposit: HASH("d43d"), pool_transfer: HASH("5dd9") },
  completed_at: "2026-10-26T02:52:09Z",
});
