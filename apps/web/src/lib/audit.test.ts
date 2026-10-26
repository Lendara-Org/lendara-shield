import type { AuditTransaction } from "@lendara/shield-api-types";

import { messages } from "@/content/messages";

import { isChange, movedAmount, otherVisible, owner, publicAmount } from "./audit";

const en = messages.en.audit;
const es = messages.es.audit;

const CAROL = `0x${"c0".repeat(32)}`;
const BOB = `0x${"b0".repeat(32)}`;
const note = (key: string, amount: string) => ({ owner_note_public_key: key, owner_stellar_account: null, amount });
const tx = (kind: AuditTransaction["kind"], ext: string, inputs: ReturnType<typeof note>[], outputs: ReturnType<typeof note>[]): AuditTransaction => ({
  ledger: 1,
  tx_hash: null,
  kind,
  ext_amount: ext,
  inputs,
  outputs,
});

describe("audit wording", () => {
  it("describes the public amount", () => {
    expect(publicAmount(en, "2", "USDC")).toBe("2 USDC into the pool");
    expect(publicAmount(en, "-0.5", "USDC")).toBe("0.5 USDC out of the pool");
    expect(publicAmount(en, "0", "USDC")).toBe("Nothing: amount hidden");
    expect(publicAmount(en, null, "USDC")).toBe("Not a pool transaction");
  });

  it("speaks Spanish too", () => {
    expect(publicAmount(es, "-0.5", "USDC")).toBe("0.5 USDC salen del pool");
    expect(publicAmount(es, "0", "USDC")).toBe("Nada: monto oculto");
    expect(es.kinds.transfer).toBe("Transferencia privada");
  });

  it("names owners by account, or flags keys Shield does not know", () => {
    const key = `0x${"a2".repeat(32)}`;
    expect(owner(en, { owner_note_public_key: key, owner_stellar_account: "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY", amount: "1" })).toBe("GBJ3H7…PFWY");
    expect(owner(en, { owner_note_public_key: key, owner_stellar_account: null, amount: "1" })).toBe("Unknown key 0xa2a2a2…a2a2");
    expect(owner(es, { owner_note_public_key: key, owner_stellar_account: null, amount: "1" })).toBe("Llave desconocida 0xa2a2a2…a2a2");
  });

  it("lists visible accounts besides the signer", () => {
    expect(
      otherVisible({
        tx_hash: "a".repeat(64),
        ledger: 1,
        created_at: "2026-10-26T03:23:47Z",
        source_account: "GSIGNER",
        is_pool_transaction: true,
        ext_amount: "2",
        visible_accounts: ["GISSUER", "GSIGNER"],
        explorer_url: "https://stellar.expert",
      }),
    ).toEqual(["GISSUER"]);
  });

  it("tells the change from what a transfer sent", () => {
    const transfer = tx("transfer", "0", [note(CAROL, "2")], [note(BOB, "0.5"), note(CAROL, "1.5")]);
    expect(isChange(transfer, note(CAROL, "1.5"))).toBe(true);
    expect(isChange(transfer, note(BOB, "0.5"))).toBe(false);
    expect(movedAmount(transfer)).toBe("0.5");
  });

  it("uses the public amount for deposits and withdrawals", () => {
    expect(movedAmount(tx("deposit", "9.3039215", [], [note(CAROL, "9.3039215")]))).toBe("9.3039215");
    expect(movedAmount(tx("withdraw", "-0.5", [note(CAROL, "1.5")], [note(CAROL, "1")]))).toBe("0.5");
  });
});
