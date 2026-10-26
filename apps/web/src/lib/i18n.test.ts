import { messages } from "@/content/messages";

import { isLang, localized, preferredLang, switchLang } from "./i18n";

describe("i18n", () => {
  it("builds localized paths", () => {
    expect(localized("es", "/deposit")).toBe("/es/deposit");
    expect(localized("en", "/")).toBe("/en");
    expect(localized("en")).toBe("/en");
  });

  it("switches the language of the current page", () => {
    expect(switchLang("/en/deposit", "es")).toBe("/es/deposit");
    expect(switchLang("/es/docs/quickstart", "en")).toBe("/en/docs/quickstart");
    expect(switchLang("/es", "en")).toBe("/en");
    expect(switchLang("/wallet", "es")).toBe("/es/wallet");
  });

  it("picks Spanish only when the browser prefers it", () => {
    expect(preferredLang("es-CR,es;q=0.9,en;q=0.8")).toBe("es");
    expect(preferredLang("en-US,es;q=0.5")).toBe("en");
    expect(preferredLang(null)).toBe("en");
    expect(isLang("es") && isLang("en") && !isLang("pt")).toBe(true);
  });

  it("has the same keys in both languages", () => {
    const shape = (value: unknown): unknown =>
      value && typeof value === "object"
        ? Object.fromEntries(Object.entries(value).map(([k, v]) => [k, shape(v)]))
        : typeof value;
    expect(shape(messages.es)).toEqual(shape(messages.en));
  });
});
