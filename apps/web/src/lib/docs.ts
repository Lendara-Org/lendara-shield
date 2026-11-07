import type { Lang } from "./i18n";

/** Page order in the sidebar. "" is the overview. */
export const DOC_SLUGS = ["", "protocol", "quickstart", "api", "scenarios", "security"] as const;
export type DocSlug = (typeof DOC_SLUGS)[number];

export const isDocSlug = (value: string): value is DocSlug => (DOC_SLUGS as readonly string[]).includes(value);

export const docHref = (lang: Lang, slug: DocSlug) => `/${lang}/docs${slug ? `/${slug}` : ""}`;

export type DocPage = { title: string; description: string; body: () => React.ReactNode };

export type DocChrome = {
  docs: string;
  previous: string;
  next: string;
  menu: string;
  copy: [label: string, copied: string];
};
