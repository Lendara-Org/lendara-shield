"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { shorten } from "@/lib/explorer";
import { LANGS, switchLang } from "@/lib/i18n";

import { Isotipo } from "./brand/isotipo";
import { useI18n } from "./i18n";
import { useSession } from "./session";

export function SiteHeader() {
  const pathname = usePathname();
  const { lang, t, href } = useI18n();
  const { wallet } = useSession();
  const links = [
    { path: "/onboard", label: t.nav.onboard },
    { path: "/deposit", label: t.nav.deposit },
    { path: "/wallet", label: t.nav.wallet },
    { path: "/audit", label: t.nav.audit },
    { path: "/docs", label: t.nav.docs },
  ];
  return (
    <header className="border-b border-line bg-surface">
      <nav aria-label={t.nav.main} className="mx-auto grid max-w-[1440px] grid-cols-[1fr_auto] items-center gap-x-6 gap-y-4 px-5 py-5 sm:px-8 xl:min-h-28 xl:grid-cols-[1fr_auto_1fr] lg:px-12 xl:py-6">
        <Link href={href("/")} className="flex items-center gap-3 text-xl sm:text-2xl font-display font-semibold tracking-normal">
          <Isotipo className="h-10 w-auto text-crimson" />
          Lendara Shield
        </Link>
        <ul className="col-span-2 row-start-2 flex flex-wrap items-center justify-center gap-x-6 gap-y-1 text-lg xl:col-span-1 xl:col-start-2 xl:row-start-1 xl:gap-x-7">
          {links.map(({ path, label }) => {
            const target = href(path);
            const current = pathname === target || (path === "/docs" && pathname.startsWith(`${target}/`));
            return (
              <li key={path}>
                <Link
                  href={target}
                  aria-current={current ? "page" : undefined}
                  className="inline-flex min-h-12 items-center border-b-2 border-transparent text-gray hover:text-ink aria-[current=page]:border-crimson aria-[current=page]:text-ink"
                >
                  {label}
                </Link>
              </li>
            );
          })}
        </ul>
        <div className="col-start-2 row-start-1 flex flex-wrap items-center justify-end gap-3 xl:col-start-3">
          <span className="hidden font-mono text-sm text-gray sm:inline">{wallet ? shorten(wallet.address, 4, 4) : t.nav.testnet}</span>
          <div role="group" aria-label={t.nav.language} className="flex rounded-md border border-line p-0.5 text-sm">
            {LANGS.map((l) => (
              <Link
                key={l}
                href={switchLang(pathname, l)}
                hrefLang={l}
                aria-current={l === lang ? "true" : undefined}
                className={`rounded px-2 py-1.5 font-mono uppercase ${l === lang ? "bg-ink text-paper" : "text-gray hover:text-ink"}`}
              >
                {l}
              </Link>
            ))}
          </div>
          <Link
            href={href("/onboard")}
            className="hidden min-h-12 items-center justify-center rounded-control bg-action-primary px-5 py-3 text-lg font-semibold whitespace-nowrap text-text-inverse transition-colors hover:bg-action-primary-hover active:bg-action-primary-pressed sm:inline-flex"
          >
            {t.landing.start}
          </Link>
        </div>
      </nav>
    </header>
  );
}
