import Link from "next/link";

const MODES = [
  {
    title: "With a ramp",
    body: "Pay in local currency. The anchor pays Shield, Shield deposits into the pool and transfers privately to your keys. Your address never appears on-chain.",
    href: "/deposit",
  },
  {
    title: "From your wallet",
    body: "Already have USDC? Deposit from your own account. Your address is visible when you deposit; everything after is private.",
    href: "/deposit?mode=direct",
  },
] as const;

export default function Home() {
  return (
    <div className="space-y-12">
      <section className="space-y-4">
        <p className="text-sm font-medium text-wine">Shield Protocol · Stellar Testnet</p>
        <h1 className="font-display text-6xl leading-none tracking-wide">KYC once. Pay privately. Disclose selectively.</h1>
        <p className="max-w-2xl text-gray">
          Lendara Shield is a compliant privacy layer for Stellar. Shield runs the KYC and the pool&apos;s allow-list,
          connects any SEP-24 anchor to a private pool, and keeps an auditor&apos;s view key, so payments are private to
          the public and accountable to the regulator.
        </p>
        <Link href="/onboard" className="inline-block rounded-md bg-ink px-5 py-3 text-sm font-medium text-paper hover:bg-ink/85">
          Start: onboard
        </Link>
      </section>

      <section aria-labelledby="modes" className="space-y-4">
        <h2 id="modes" className="text-lg font-semibold">
          Two ways in, one core
        </h2>
        <div className="grid gap-4 sm:grid-cols-2">
          {MODES.map((mode) => (
            <Link key={mode.title} href={mode.href} className="rounded-lg border border-line bg-surface p-6 transition-colors hover:border-ink/40">
              <h3 className="mb-2 font-semibold">{mode.title}</h3>
              <p className="text-sm text-gray">{mode.body}</p>
            </Link>
          ))}
        </div>
      </section>
    </div>
  );
}
