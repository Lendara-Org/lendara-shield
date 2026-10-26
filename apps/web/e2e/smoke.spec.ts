import { expect, test } from "@playwright/test";

import { info } from "../src/test/fixtures";

const API = "http://localhost:8080";

test.beforeEach(async ({ page }) => {
  await page.route(`${API}/v1/info`, (route) => route.fulfill({ json: info, headers: { "access-control-allow-origin": "*" } }));
});

test("landing → onboard → deposit, and a clear message without Freighter", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toContainText("KYC once");

  await page.getByRole("link", { name: "Start: onboard" }).click();
  await expect(page).toHaveURL(/\/onboard$/);
  await page.getByRole("button", { name: "Connect Freighter" }).click();
  await expect(page.getByRole("main").getByRole("alert")).toContainText("Freighter is not installed");

  await page.getByRole("navigation", { name: "Main" }).getByRole("link", { name: "Deposit" }).click();
  await expect(page).toHaveURL(/\/deposit$/);
  await expect(page.getByRole("button", { name: "Connect Freighter" })).toBeVisible();
});

test("says when Shield's API is down", async ({ page }) => {
  await page.route(`${API}/v1/info`, (route) => route.abort());
  await page.goto("/wallet");
  await expect(page.getByRole("main").getByRole("alert")).toContainText("unreachable");
});
