import Link from "next/link";

import { DOC_SLUGS, type DocChrome, type DocPage, type DocSlug, docHref } from "@/lib/docs";
import type { Lang } from "@/lib/i18n";

type Props = {
  lang: Lang;
  slug: DocSlug;
  page: DocPage;
  chrome: DocChrome;
  titles: Record<DocSlug, string>;
};

/** Sidebar + article, inside the site's header and footer. */
export function DocsLayout({ lang, slug, page, chrome, titles }: Props) {
  const index = DOC_SLUGS.indexOf(slug);
  const prev = index > 0 ? DOC_SLUGS[index - 1] : undefined;
  const next = index < DOC_SLUGS.length - 1 ? DOC_SLUGS[index + 1] : undefined;

  const nav = (
    <ul className="flex flex-col gap-2 text-lg">
      {DOC_SLUGS.map((s) => (
        <li key={s}>
          <Link
            href={docHref(lang, s)}
            aria-current={s === slug ? "page" : undefined}
            className={`block rounded-lg px-4 py-3 transition ${
              s === slug ? "bg-surface font-medium text-ink" : "text-gray hover:text-ink"
            }`}
          >
            {titles[s]}
          </Link>
        </li>
      ))}
    </ul>
  );

  return (
    <main className="mx-auto grid max-w-[1440px] grid-cols-[minmax(0,1fr)] gap-12 px-5 py-12 sm:px-8 lg:px-12 lg:grid-cols-[240px_minmax(0,1fr)] lg:py-24">
      <aside className="lg:sticky lg:top-6 lg:self-start">
        <details className="rounded-lg border border-line bg-surface lg:hidden">
          <summary className="cursor-pointer px-4 py-3 text-base font-medium">{chrome.menu}</summary>
          <div className="border-t border-line p-2">{nav}</div>
        </details>
        <nav aria-label={chrome.docs} className="hidden lg:block">
          {nav}
        </nav>
      </aside>

      <article className="min-w-0 text-lg leading-8">
        <h1 className="text-[40px] leading-[46px] font-semibold tracking-tight text-ink sm:text-[52px] sm:leading-[60px] lg:text-[64px] lg:leading-[72px]">{page.title}</h1>
        <p className="mt-4 text-xl leading-8 text-gray">{page.description}</p>
        <div className="mt-12">{page.body()}</div>

        <nav className="mt-16 grid gap-3 border-t border-line pt-6 sm:grid-cols-2">
          {prev !== undefined ? (
            <Link href={docHref(lang, prev)} className="rounded-lg border border-line bg-surface p-4 transition hover:border-ink">
              <span className="text-sm text-gray">{chrome.previous}</span>
              <span className="mt-1 block font-medium text-ink">{titles[prev]}</span>
            </Link>
          ) : (
            <span />
          )}
          {next !== undefined && (
            <Link
              href={docHref(lang, next)}
              className="rounded-lg border border-line bg-surface p-4 text-right transition hover:border-ink"
            >
              <span className="text-sm text-gray">{chrome.next}</span>
              <span className="mt-1 block font-medium text-ink">{titles[next]}</span>
            </Link>
          )}
        </nav>
      </article>
    </main>
  );
}
