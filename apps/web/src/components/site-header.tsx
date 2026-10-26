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
      <nav aria-label={t.nav.main} className="mx-auto flex max-w-[1440px] flex-wrap items-center gap-x-6 gap-y-2 px-5 py-5 sm:px-8 lg:px-12 lg:py-6">
        <Link href={href("/")} className="flex items-center gap-2.5 text-lg font-display font-semibold tracking-normal">
          <Isotipo className="h-8 w-auto text-crimson" />
          Lendara Shield
        </Link>
        <ul className="flex flex-wrap gap-4 text-base lg:gap-6">
          {links.map(({ path, label }) => {
            const target = href(path);
            const current = pathname === target || (path === "/docs" && pathname.startsWith(`${target}/`));
            return (
              <li key={path}>
                <Link
                  href={target}
                  aria-current={current ? "page" : undefined}
                  className="border-b-2 border-transparent pb-0.5 text-gray hover:text-ink aria-[current=page]:border-crimson aria-[current=page]:text-ink"
                >
                  {label}
                </Link>
              </li>
            );
          })}
        </ul>
        <div className="ml-auto flex items-center gap-3">
          <span className="font-mono text-xs text-gray">{wallet ? shorten(wallet.address, 4, 4) : t.nav.testnet}</span>
          <div role="group" aria-label={t.nav.language} className="flex rounded-md border border-line p-0.5 text-xs">
            {LANGS.map((l) => (
              <Link
                key={l}
                href={switchLang(pathname, l)}
                hrefLang={l}
                aria-current={l === lang ? "true" : undefined}
                className={`rounded px-2 py-0.5 font-mono uppercase ${l === lang ? "bg-ink text-paper" : "text-gray hover:text-ink"}`}
              >
                {l}
              </Link>
            ))}
          </div>
        </div>
      </nav>
    </header>
  );
}
