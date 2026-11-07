/**
 * Types for the Lendara Shield API, generated from `openapi.yaml`.
 *
 * Do not edit `schema.gen.ts` by hand: change `openapi.yaml` and run
 * `make codegen`.
 */
import type { components, operations, paths } from "./schema.gen.ts";

export type { components, operations, paths };

type Schemas = components["schemas"];

export type ApiError = Schemas["Error"];
export type Health = Schemas["Health"];
export type Info = Schemas["Info"];
export type PoolInfo = Schemas["PoolInfo"];
export type KycScenario = Schemas["KycScenario"];
export type OnboardingRequest = Schemas["OnboardingRequest"];
export type ShieldUser = Schemas["ShieldUser"];
export type AspStatus = ShieldUser["asp_status"];
export type AspActionRequest = Schemas["AspActionRequest"];
export type CreateIntentRequest = Schemas["CreateIntentRequest"];
export type Intent = Schemas["Intent"];
export type IntentStatus = Schemas["IntentStatus"];
export type AuditNote = Schemas["AuditNote"];
export type AuditTransaction = Schemas["AuditTransaction"];
export type AuditTransactionList = Schemas["AuditTransactionList"];
export type PublicTransaction = Schemas["PublicTransaction"];

/** Intent states after which polling can stop. */
export const FINAL_INTENT_STATUSES = [
  "completed",
  "rejected",
  "expired",
  "error",
] as const satisfies readonly IntentStatus[];

export type FinalIntentStatus = (typeof FINAL_INTENT_STATUSES)[number];

/** True when the intent will not change any more. */
export function isFinalIntentStatus(status: IntentStatus): status is FinalIntentStatus {
  return (FINAL_INTENT_STATUSES as readonly IntentStatus[]).includes(status);
}
