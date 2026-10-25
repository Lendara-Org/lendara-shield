/**
 * Compile-time checks that the generated types match the contract the web
 * and the API rely on. `pnpm typecheck` fails if any expectation breaks.
 */
import type {
  CreateIntentRequest,
  Intent,
  IntentStatus,
  OnboardingRequest,
  ShieldUser,
} from "../src/index.ts";
import { isFinalIntentStatus } from "../src/index.ts";

const SAMPLE_POOL_FIELD = "0x05a8c36ec6273fca4fed02573a4030279b2ec26f79ebf9cc784d8e6313f8fd74";
const ACCOUNT = "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY";

// Onboarding sends public pool keys and the ASP leaf; scenario is optional.
export const onboarding = {
  stellar_account: ACCOUNT,
  note_public_key: SAMPLE_POOL_FIELD,
  encryption_public_key: SAMPLE_POOL_FIELD,
  asp_leaf: SAMPLE_POOL_FIELD,
} satisfies OnboardingRequest;

// @ts-expect-error spending keys are never part of the request.
export const leaksSecret: OnboardingRequest = { ...onboarding, note_private_key: SAMPLE_POOL_FIELD };

// @ts-expect-error scenario only accepts the documented outcomes.
export const badScenario: OnboardingRequest = { ...onboarding, scenario: "maybe" };

// Amounts are strings, never numbers.
export const intentRequest = {
  mode: "ramp",
  stellar_account: ACCOUNT,
  fiat_currency: "CRC",
  amount: "100.00",
} satisfies CreateIntentRequest;

// @ts-expect-error amounts must be decimal strings.
export const numericAmount: CreateIntentRequest = { ...intentRequest, amount: 100 };

// @ts-expect-error only ramp mode creates intents.
export const directIntent: CreateIntentRequest = { ...intentRequest, mode: "direct" };

// Every hash slot is present and nullable until its step completes.
export const freshIntent = {
  id: "3f1c2b8e-8a4d-4c1e-9b7a-2f5d6e7c8a90",
  mode: "ramp",
  status: "awaiting_anchor",
  fiat_currency: "CRC",
  amount_fiat: "100.00",
  asset_code: "USDC",
  amount_asset: null,
  interactive_url: "https://anchor.example/sep24/interactive?token=abc",
  transactions: { intake_payment: null, pool_deposit: null, pool_transfer: null },
  message: null,
  created_at: "2026-10-26T12:00:00Z",
  updated_at: "2026-10-26T12:00:00Z",
  completed_at: null,
} satisfies Intent;

export const user = {
  stellar_account: ACCOUNT,
  kyc_status: "approved",
  asp_status: "pending",
  last_asp_tx_hash: null,
  created_at: "2026-10-26T12:00:00Z",
  updated_at: "2026-10-26T12:00:00Z",
} satisfies ShieldUser;

// Exhaustiveness: adding a status to the contract breaks this switch.
export function label(status: IntentStatus): string {
  switch (status) {
    case "created":
    case "awaiting_anchor":
      return "Waiting for the anchor";
    case "funds_received":
    case "shielding":
    case "transferring":
      return "Shielding";
    case "completed":
      return "Done";
    case "rejected":
    case "expired":
    case "error":
      return "Failed";
    default: {
      const unreachable: never = status;
      return unreachable;
    }
  }
}

export const finalCheck: boolean = isFinalIntentStatus(freshIntent.status);
