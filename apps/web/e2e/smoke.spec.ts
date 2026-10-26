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

test("the audit compares the explorer with the auditor", async ({ page }) => {
  const cors = { "access-control-allow-origin": "*", "access-control-allow-headers": "authorization" };
  const hash = "2fe1".repeat(16);
  const intake = "GCRN35RTRRZAIFFEMOTZFV7PNPMFOTDNXQKLMOQFSMOBU3LAON3A6NJC";
  const carol = "GCWQUSCIB2S4LNW6FIOKL2N6BLFKAQ365A43IZVWJBR2ANAEDSIG3GQK";
  await page.route(`${API}/v1/audit/transactions?limit=20`, (route) =>
    route.request().method() === "OPTIONS"
      ? route.fulfill({ status: 204, headers: cors })
      : route.fulfill({
          headers: cors,
          json: {
            transactions: [
              {
                ledger: 5392310,
                tx_hash: hash,
                kind: "transfer",
                ext_amount: "0",
                inputs: [{ owner_note_public_key: `0x${"1d".repeat(32)}`, owner_stellar_account: intake, amount: "9.3039215" }],
                outputs: [{ owner_note_public_key: `0x${"c0".repeat(32)}`, owner_stellar_account: carol, amount: "9.3039215" }],
              },
            ],
          },
        }),
  );
  await page.route(`${API}/v1/audit/public/${hash}`, (route) =>
    route.fulfill({
      headers: cors,
      json: {
        tx_hash: hash,
        ledger: 5392310,
        created_at: "2026-10-26T03:30:00Z",
        source_account: intake,
        is_pool_transaction: true,
        ext_amount: "0",
        visible_accounts: [intake],
        explorer_url: `https://stellar.expert/explorer/testnet/tx/${hash}`,
      },
    }),
  );

  await page.goto("/audit");
  await page.getByLabel("Auditor token").fill("auditor-token");
  await page.getByRole("button", { name: "Open the audit" }).click();
  await expect(page.getByText("Nothing: amount hidden")).toBeVisible();
  await expect(page.getByText("GCWQUS…3GQK")).toBeVisible();
});

test("switches language and reads the docs", async ({ page }) => {
  await page.goto("/en");
  await page.getByRole("group", { name: "Language" }).getByRole("link", { name: "es" }).click();
  await expect(page).toHaveURL(/\/es$/);
  await expect(page.getByRole("heading", { level: 1 })).toContainText("KYC una vez");

  await page.getByRole("navigation", { name: "Principal" }).getByRole("link", { name: "Docs" }).click();
  await expect(page).toHaveURL(/\/es\/docs$/);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Introducción");
  await page.getByRole("link", { name: /Siguiente/ }).click();
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Shield Protocol");

  // Old links still work, in English.
  await page.goto("/onboard");
  await expect(page).toHaveURL(/\/en\/onboard$/);
});

test("the visual identity stays readable and usable across viewport widths", async ({ page }) => {
  for (const width of [375, 768, 1280, 1920]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto("/es");
    await page.evaluate(() => document.fonts.ready);
    if (width === 1920) {
      await expect(page.getByRole("main")).toHaveCSS("max-width", "1440px");
      await expect(page.getByRole("heading", { level: 1 })).toHaveCSS("font-size", "76px");
    }
    await expect(page.getByRole("heading", { level: 1 })).toHaveCSS("font-weight", "600");
    await expect(page.getByRole("link", { name: "Empezar: registro" })).toHaveCSS("background-color", "rgb(194, 24, 82)");
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.getByRole("link", { name: "Empezar: registro" }).click();
    const connect = page.getByRole("button", { name: "Conectar Freighter" });
    await expect(connect).toBeVisible();
    await expect(connect).toHaveCSS("min-height", "48px");
    await expect(connect).toHaveCSS("border-radius", "10px");
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  }
});
