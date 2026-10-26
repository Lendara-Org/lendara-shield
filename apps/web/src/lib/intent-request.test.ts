import { intentMessage, signedIntentRequest } from "./intent-request";

const ALICE = "GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC";

describe("intentMessage", () => {
  it("matches the API's text byte for byte", () => {
    // Same vector as `the_intent_message_binds_the_amount_and_a_single_use_request` in apps/api.
    expect(intentMessage(ALICE, "CRC", "5000", "0194F3A2-7C1E-7D3B-9A4F-2B6C8E1D5F70", 1_793_016_000)).toBe(
      "Lendara Shield intent\nnetwork: testnet\n" +
        `account: ${ALICE}\n` +
        "fiat_currency: CRC\namount: 5000\n" +
        "request_id: 0194f3a2-7c1e-7d3b-9a4f-2b6c8e1d5f70\nissued_at: 1793016000",
    );
  });
});

describe("signedIntentRequest", () => {
  it("signs the exact request it returns", async () => {
    const signMessage = vi.fn(() => Promise.resolve("c2lnbmVk"));
    const request = await signedIntentRequest(
      { address: ALICE, signMessage },
      "CRC",
      "5000",
      () => 1_793_016_000_999,
      () => "0194F3A2-7C1E-7D3B-9A4F-2B6C8E1D5F70",
    );

    expect(request).toEqual({
      mode: "ramp",
      stellar_account: ALICE,
      fiat_currency: "CRC",
      amount: "5000",
      request_id: "0194f3a2-7c1e-7d3b-9a4f-2b6c8e1d5f70",
      issued_at: 1_793_016_000,
      signature: "c2lnbmVk",
    });
    expect(signMessage).toHaveBeenCalledWith(
      intentMessage(ALICE, "CRC", "5000", request.request_id, request.issued_at),
    );
  });

  it("uses a new request id every time", async () => {
    const wallet = { address: ALICE, signMessage: () => Promise.resolve("c2lnbmVk") };
    const [a, b] = await Promise.all([
      signedIntentRequest(wallet, "CRC", "10"),
      signedIntentRequest(wallet, "CRC", "10"),
    ]);
    expect(a.request_id).not.toBe(b.request_id);
  });
});
