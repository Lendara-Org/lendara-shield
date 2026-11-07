import { ENC_KEY, NOTE_KEY } from "@/test/fixtures";

import { decodeRecipient, encodeRecipient } from "./recipient";

const keys = { notePublicKey: NOTE_KEY, encryptionPublicKey: ENC_KEY };

describe("private receiving address", () => {
  it("round-trips the two public keys", () => {
    const address = encodeRecipient(keys);
    expect(address).toBe(`shield:${NOTE_KEY}:${ENC_KEY}`);
    expect(decodeRecipient(address)).toEqual(keys);
  });

  it("accepts surrounding spaces and upper case", () => {
    expect(decodeRecipient(`  ${encodeRecipient(keys).toUpperCase()} `)).toEqual(keys);
  });

  it.each([
    ["empty", ""],
    ["a Stellar account", "GBJ3H74T4LB23V3WKE46BSWMZVOH4YWX2P5ZLQTY4EZXH3YAZZUCPFWY"],
    ["another prefix", `pool:${NOTE_KEY}:${ENC_KEY}`],
    ["one key", `shield:${NOTE_KEY}`],
    ["extra parts", `shield:${NOTE_KEY}:${ENC_KEY}:x`],
    ["short key", `shield:0x1234:${ENC_KEY}`],
    ["non-hex key", `shield:0x${"zz".repeat(32)}:${ENC_KEY}`],
  ])("rejects %s", (_, input) => {
    expect(decodeRecipient(input)).toBeNull();
  });
});
