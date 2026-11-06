import { bootnodeOption } from "./bootnode";

describe("bootnodeOption", () => {
  it("passes a configured bootnode to the SDK without probing the RPC", async () => {
    const isRequired = vi.fn(async () => true);
    await expect(bootnodeOption("https://shield.example/bootnode", isRequired)).resolves.toEqual({
      bootnodeUrl: "https://shield.example/bootnode",
    });
    expect(isRequired).not.toHaveBeenCalled();
  });

  it("needs no bootnode while the RPC holds the whole pool history", async () => {
    await expect(bootnodeOption(undefined, async () => false)).resolves.toEqual({});
  });

  it("fails clearly when the pool is older than the RPC window and none is configured", async () => {
    await expect(bootnodeOption(undefined, async () => true)).rejects.toThrow(/NEXT_PUBLIC_BOOTNODE_URL/);
  });

  it("lets a failed probe surface", async () => {
    await expect(
      bootnodeOption(undefined, async () => {
        throw new Error("rpc down");
      }),
    ).rejects.toThrow("rpc down");
  });
});
