import { HttpResponse, http } from "msw";

import { ACCOUNT, ENC_KEY, LEAF, NOTE_KEY, info, intent, user } from "@/test/fixtures";
import { apiError, server, url } from "@/test/server";

import { ApiRequestError, api } from "./api";

describe("api", () => {
  it("reads the public info", async () => {
    expect(await api.info()).toEqual(info);
  });

  it("posts onboarding as JSON", async () => {
    let body: unknown;
    server.use(
      http.post(url("/v1/onboarding"), async ({ request }) => {
        body = await request.json();
        return HttpResponse.json(user({ asp_status: "pending" }), { status: 201 });
      }),
    );
    const request = {
      stellar_account: ACCOUNT,
      note_public_key: NOTE_KEY,
      encryption_public_key: ENC_KEY,
      asp_leaf: LEAF,
      signature: "c2lnbmF0dXJl",
      scenario: "approved" as const,
    };
    expect((await api.onboard(request)).asp_status).toBe("pending");
    expect(body).toEqual(request);
  });

  it("returns null for a user who never onboarded", async () => {
    server.use(http.get(url(`/v1/users/${ACCOUNT}`), () => apiError(404, "not_found", "no such user")));
    expect(await api.user(ACCOUNT)).toBeNull();
  });

  it("keeps the API's error code and message", async () => {
    server.use(http.post(url("/v1/intents"), () => apiError(409, "user_not_allowed", "Not in the allow-list")));
    const err = await api
      .createIntent({
        mode: "ramp",
        stellar_account: ACCOUNT,
        fiat_currency: "CRC",
        amount: "5000",
        request_id: "0194f3a2-7c1e-7d3b-9a4f-2b6c8e1d5f70",
        issued_at: 1_793_016_000,
        signature: "c2lnbmVk",
      })
      .catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiRequestError);
    expect(err).toMatchObject({ status: 409, code: "user_not_allowed", message: "Not in the allow-list" });
  });

  it("reports errors that are not the API's shape", async () => {
    server.use(http.get(url(`/v1/intents/x`), () => new HttpResponse("bad gateway", { status: 502 })));
    await expect(api.intent("x")).rejects.toMatchObject({ status: 502, code: "unexpected" });
  });

  it("reports an unreachable API", async () => {
    server.use(http.get(url("/v1/intents/y"), () => HttpResponse.error()));
    await expect(api.intent("y")).rejects.toMatchObject({ status: 0, code: "network" });
  });

  it("escapes path parameters", async () => {
    server.use(http.get(url("/v1/intents/a%2Fb"), () => HttpResponse.json(intent())));
    expect((await api.intent("a/b")).status).toBe("awaiting_anchor");
  });
});
