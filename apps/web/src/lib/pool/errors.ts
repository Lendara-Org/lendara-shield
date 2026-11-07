/**
 * Why a pool operation failed, in terms the pages can explain. The SDK
 * reports failures as English text from deep inside it (planner, Freighter,
 * the token contract's simulation); this maps the ones a user can act on and
 * leaves the rest as `other`, shown with the SDK's own message.
 */

export type PoolOperation = "deposit" | "transfer" | "withdraw";

export type PoolFailureKind =
  /** The user declined to sign in Freighter. */
  | "rejected"
  /** The private balance cannot cover the amount (no combination of notes). */
  | "insufficientPrivate"
  /** The account's public balance of the asset cannot cover the deposit. */
  | "insufficientPublic"
  /** The account has no trustline to the asset. */
  | "trustlineMissing"
  /** The pool has not seen the user's allow-list entry yet. */
  | "aspNotReady"
  | "other";

/** SDK text when a wallet request was declined (Freighter's SEP-43 "user rejected"). */
const REJECTED = /rejected by user|user declined|user rejected/i;
/** The SDK's note planner could not reach the amount with the user's unspent notes. */
const NO_NOTES = /no combination possible/i;
/** Stellar Asset Contract errors: #10 balance out of range, #13 missing trustline. */
const SAC_BALANCE = /Error\(Contract, #10\)|resulting balance is not within the allowed range/;
const SAC_TRUSTLINE = /Error\(Contract, #13\)|trustline entry is missing/;

export function classifyPoolFailure(operation: PoolOperation, status: string, message: string): PoolFailureKind {
  if (status === "aspNotReady") return "aspNotReady";
  if (REJECTED.test(message)) return "rejected";
  if (SAC_TRUSTLINE.test(message)) return "trustlineMissing";
  if (operation !== "deposit" && NO_NOTES.test(message)) return "insufficientPrivate";
  // Only a deposit spends the account's own tokens; on a withdraw the pool pays.
  if (operation === "deposit" && SAC_BALANCE.test(message)) return "insufficientPublic";
  return "other";
}

/** SEP-43 code Freighter uses when the user declines. */
const USER_REJECTED_CODE = -4;

/** Whether `err`, thrown outside a pool result (e.g. deriving keys), is the user declining in Freighter. */
export function isUserRejection(err: unknown): boolean {
  if (!(err instanceof Error)) return false;
  const code = (err as Error & { code?: unknown }).code;
  return code === USER_REJECTED_CODE || REJECTED.test(err.message);
}
