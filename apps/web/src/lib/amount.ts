/**
 * Token amounts. Pool and Stellar amounts are integers of stroops (7
 * decimals); the UI shows and accepts decimal strings. Never a float.
 */

export const DECIMALS = 7;
const SCALE = 10n ** BigInt(DECIMALS);
const DECIMAL = /^(0|[1-9]\d*)(?:\.(\d{1,7}))?$/;

/** Parses a positive decimal like `"9.5"` into stroops; `null` if invalid or zero. */
export function parseAmount(input: string): bigint | null {
  const match = DECIMAL.exec(input.trim());
  if (!match) return null;
  const [, whole = "0", fraction = ""] = match;
  const stroops = BigInt(whole) * SCALE + BigInt(fraction.padEnd(DECIMALS, "0"));
  return stroops > 0n ? stroops : null;
}

/** Formats stroops as a decimal without trailing zeros (`93039215n` → `"9.3039215"`). */
export function formatAmount(stroops: bigint): string {
  const sign = stroops < 0n ? "-" : "";
  const abs = stroops < 0n ? -stroops : stroops;
  const whole = abs / SCALE;
  const fraction = (abs % SCALE).toString().padStart(DECIMALS, "0").replace(/0+$/, "");
  return `${sign}${whole}${fraction ? `.${fraction}` : ""}`;
}
