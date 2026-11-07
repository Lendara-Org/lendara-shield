import { render, screen } from "@testing-library/react";

import { DocsLayout } from "@/components/docs/docs-layout";
import { DOC_SLUGS, type DocSlug } from "@/lib/docs";
import { LANGS } from "@/lib/i18n";

import { docs } from ".";

describe("docs", () => {
  it.each(LANGS)("has every page in %s, each one rendering", (lang) => {
    const { chrome, pages } = docs[lang];
    const titles = Object.fromEntries(DOC_SLUGS.map((s) => [s, pages[s].title])) as Record<DocSlug, string>;
    for (const slug of DOC_SLUGS) {
      const page = pages[slug];
      expect(page.title && page.description).toBeTruthy();
      const { unmount } = render(<DocsLayout lang={lang} slug={slug} page={page} chrome={chrome} titles={titles} />);
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(page.title);
      unmount();
    }
  });

  it("links every page inside its own language", () => {
    for (const lang of LANGS) {
      const { chrome, pages } = docs[lang];
      const titles = Object.fromEntries(DOC_SLUGS.map((s) => [s, pages[s].title])) as Record<DocSlug, string>;
      for (const slug of DOC_SLUGS) {
        const { container, unmount } = render(
          <DocsLayout lang={lang} slug={slug} page={pages[slug]} chrome={chrome} titles={titles} />,
        );
        for (const link of container.querySelectorAll("a[href^='/']")) {
          const href = link.getAttribute("href") ?? "";
          expect(href === "/api-reference.html" || href.startsWith(`/${lang}`), `${lang}/${slug}: ${href}`).toBe(true);
        }
        unmount();
      }
    }
  });
});
