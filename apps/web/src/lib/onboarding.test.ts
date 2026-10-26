import { onboardingMessage } from "./onboarding";

describe("onboardingMessage", () => {
  it("matches the API's text byte for byte", () => {
    // Same vector as `the_message_names_the_account_and_every_key` in apps/api.
    expect(
      onboardingMessage("GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC", {
        notePublicKey: `0x${"A2".repeat(32)}`,
        encryptionPublicKey: `0x${"01".repeat(32)}`,
        aspLeaf: `0x${"05".repeat(32)}`,
      }),
    ).toBe(
      "Lendara Shield onboarding\nnetwork: testnet\n" +
        "account: GDZOAGBQFY65MGK5Z6RBBFCHPYB2PJDJW3UV47IWK53UQF4LFSUP6XZC\n" +
        `note_public_key: 0x${"a2".repeat(32)}\nencryption_public_key: 0x${"01".repeat(32)}\nasp_leaf: 0x${"05".repeat(32)}`,
    );
  });
});
