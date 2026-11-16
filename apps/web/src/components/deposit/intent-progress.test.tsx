import { render, screen } from "@testing-library/react";
import { HttpResponse, http } from "msw";

import { I18nProvider } from "@/components/i18n";
import { HASH, completed, intent } from "@/test/fixtures";
import { server, url } from "@/test/server";

import { IntentProgress } from "./intent-progress";

vi.mock("@/lib/config", async (original) => ({ ...(await original<object>()), INTENT_POLL_MS: 10 }));

const shielding = intent({ status: "shielding", transactions: { intake_payment: HASH("2fc5"), pool_deposit: null, pool_transfer: null } });

function renderProgress(initial = shielding) {
  return render(
    <I18nProvider lang="en">
      <IntentProgress initial={initial} />
    </I18nProvider>,
  );
}

describe("IntentProgress", () => {
  it("shows that the step in flight is working, and only that one", async () => {
    server.use(http.get(url(`/v1/intents/${shielding.id}`), () => HttpResponse.json(shielding)));
    renderProgress();

    const steps = screen.getAllByRole("listitem");
    expect(steps.map((s) => s.dataset.state)).toEqual(["done", "done", "current", "pending"]);
    expect(screen.getAllByTestId("step-spinner")).toHaveLength(1);
    expect(steps[2]).toContainElement(screen.getByTestId("step-spinner"));
    expect(steps[2]).toHaveTextContent("Proving and sending the deposit to the pool…");
    expect(screen.getByText(/This takes about a minute/)).toBeInTheDocument();
  });

  it("stops the loading once the intent completes", async () => {
    server.use(http.get(url(`/v1/intents/${completed.id}`), () => HttpResponse.json(completed)));
    renderProgress({ ...completed, status: "transferring" });

    expect(await screen.findByText(/are now private/)).toBeInTheDocument();
    expect(screen.queryByTestId("step-spinner")).not.toBeInTheDocument();
    expect(screen.queryByText(/This takes about a minute/)).not.toBeInTheDocument();
  });

  it("stops the loading when the intent fails", () => {
    renderProgress(intent({ status: "error", message: "pool unavailable", transactions: { intake_payment: HASH("2fc5"), pool_deposit: null, pool_transfer: null } }));

    expect(screen.getAllByRole("listitem").map((s) => s.dataset.state)).toEqual(["done", "done", "failed", "pending"]);
    expect(screen.queryByTestId("step-spinner")).not.toBeInTheDocument();
    expect(screen.queryByText(/This takes about a minute/)).not.toBeInTheDocument();
  });
});
