/** Languages of the site. Every page lives under `/<lang>/…`. */

export const LANGS = ["en", "es"] as const;
export type Lang = (typeof LANGS)[number];

export const DEFAULT_LANG: Lang = "en";

export const isLang = (value: string): value is Lang => (LANGS as readonly string[]).includes(value);

/** `/en/onboard` from `("en", "/onboard")`; `path` starts with `/` or is empty. */
export const localized = (lang: Lang, path = "") => `/${lang}${path === "/" ? "" : path}`;

/** The same page in another language: swaps the first path segment. */
export function switchLang(pathname: string, lang: Lang): string {
  const [, first, ...rest] = pathname.split("/");
  return first !== undefined && isLang(first) ? `/${[lang, ...rest].join("/")}` : localized(lang, pathname);
}

/** `es` when the browser prefers Spanish, else English. */
export function preferredLang(acceptLanguage: string | null): Lang {
  return (acceptLanguage ?? "").trim().toLowerCase().startsWith("es") ? "es" : DEFAULT_LANG;
}
