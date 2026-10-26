import Link from "next/link";

import { localized } from "@/lib/i18n";
import { type LangParams, pageCopy } from "@/lib/page-params";

export default async function Home(props: LangParams) {
  const { lang, t } = await pageCopy(props);
  const { landing } = t;
  const modes = [
    { ...landing.ramp, href: localized(lang, "/deposit") },
    { ...landing.direct, href: localized(lang, "/deposit?mode=direct") },
  ];
  return (
    <main className="mx-auto max-w-4xl space-y-12 px-5 py-10 sm:py-16">
      <section className="space-y-4">
        <p className="text-sm font-medium text-wine">{landing.eyebrow}</p>
        <h1 className="font-display text-[40px] leading-[46px] font-semibold tracking-[-0.035em] sm:text-[60px] sm:leading-[68px] sm:tracking-[-0.045em]">{landing.title}</h1>
        <p className="max-w-2xl text-gray">{landing.intro}</p>
        <div className="flex flex-wrap gap-3">
          <Link
            href={localized(lang, "/onboard")}
            className="inline-flex min-h-12 items-center rounded-full bg-action-primary px-6 py-3 text-base font-semibold text-text-inverse transition-colors hover:bg-action-primary-hover active:bg-action-primary-pressed"
          >
            {landing.start}
          </Link>
          <Link
            href={localized(lang, "/docs")}
            className="inline-flex min-h-12 items-center rounded-full border border-border-control bg-surface px-6 py-3 text-base font-semibold transition-colors hover:bg-action-secondary-hover active:bg-action-secondary-pressed"
          >
            {landing.readDocs}
          </Link>
        </div>
      </section>

      <section aria-labelledby="modes" className="space-y-4">
        <h2 id="modes" className="text-2xl leading-8 font-semibold">
          {landing.modesTitle}
        </h2>
        <div className="grid gap-4 sm:grid-cols-2">
          {modes.map((mode) => (
            <Link
              key={mode.title}
              href={mode.href}
              className="rounded-2xl border border-line bg-surface p-6 transition-colors hover:border-ink/40"
            >
              <h3 className="mb-2 font-semibold">{mode.title}</h3>
              <p className="text-sm text-gray">{mode.body}</p>
            </Link>
          ))}
        </div>
      </section>

      <section aria-labelledby="developers" className="space-y-2">
        <h2 id="developers" className="text-2xl leading-8 font-semibold">
          {landing.devTitle}
        </h2>
        <p className="max-w-2xl text-sm text-gray">
          {landing.devBody}{" "}
          <Link href={localized(lang, "/docs")} className="font-medium text-ink underline">
            {landing.readDocs} →
          </Link>
        </p>
      </section>
    </main>
  );
}
