"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { shorten } from "@/lib/explorer";

import { Isotipo } from "./brand/isotipo";
import { useSession } from "./session";

const LINKS = [
  { href: "/onboard", label: "Onboard" },
  { href: "/deposit", label: "Deposit" },
  { href: "/wallet", label: "Wallet" },
] as const;

export function SiteHeader() {
  const pathname = usePathname();
  const { wallet } = useSession();
  return (
    <header className="border-b border-line bg-surface">
      <nav aria-label="Main" className="mx-auto flex max-w-4xl items-center gap-6 px-4 py-3">
        <Link href="/" className="flex items-center gap-2 font-semibold">
          <Isotipo className="h-6 w-auto" />
          Lendara Shield
        </Link>
        <ul className="flex gap-4 text-sm">
          {LINKS.map(({ href, label }) => (
            <li key={href}>
              <Link
                href={href}
                aria-current={pathname === href ? "page" : undefined}
                className="border-b-2 border-transparent pb-0.5 text-gray hover:text-ink aria-[current=page]:border-crimson aria-[current=page]:text-ink"
              >
                {label}
              </Link>
            </li>
          ))}
        </ul>
        <span className="ml-auto font-mono text-xs text-gray">{wallet ? shorten(wallet.address, 4, 4) : "Testnet"}</span>
      </nav>
    </header>
  );
}
