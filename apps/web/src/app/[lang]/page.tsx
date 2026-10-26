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
    <main className="mx-auto max-w-[1440px] space-y-16 px-5 py-12 sm:px-8 sm:py-20 lg:space-y-24 lg:px-12 lg:py-24">
      <section className="space-y-6 lg:space-y-8">
        <p className="text-base font-medium text-wine lg:text-lg">{landing.eyebrow}</p>
        <h1 className="font-display text-[40px] leading-[46px] font-semibold tracking-[-0.035em] sm:text-[64px] sm:leading-[72px] lg:max-w-[1200px] lg:text-[76px] lg:leading-[84px] sm:tracking-[-0.045em]">{landing.title}</h1>
        <p className="max-w-[880px] text-lg leading-8 text-gray lg:text-[22px] lg:leading-[34px]">{landing.intro}</p>
        <div className="flex flex-wrap gap-3">
          <Link
            href={localized(lang, "/onboard")}
            className="inline-flex min-h-14 items-center rounded-full bg-action-primary px-7 py-4 text-lg font-semibold text-text-inverse transition-colors hover:bg-action-primary-hover active:bg-action-primary-pressed"
          >
            {landing.start}
          </Link>
          <Link
            href={localized(lang, "/docs")}
            className="inline-flex min-h-14 items-center rounded-full border border-border-control bg-surface px-7 py-4 text-lg font-semibold transition-colors hover:bg-action-secondary-hover active:bg-action-secondary-pressed"
          >
            {landing.readDocs}
          </Link>
        </div>
      </section>

      <section aria-labelledby="modes" className="space-y-8 border-t border-line pt-12 lg:pt-16">
        <h2 id="modes" className="text-[30px] leading-[38px] font-semibold lg:text-[40px] lg:leading-[48px]">
          {landing.modesTitle}
        </h2>
        <div className="grid gap-6 sm:grid-cols-2 lg:gap-8">
          {modes.map((mode) => (
            <Link
              key={mode.title}
              href={mode.href}
              className="rounded-3xl border border-line bg-surface p-6 lg:p-10 transition-colors hover:border-ink/40"
            >
              <h3 className="mb-4 text-2xl leading-8 font-semibold">{mode.title}</h3>
              <p className="text-lg leading-8 text-gray">{mode.body}</p>
            </Link>
          ))}
        </div>
      </section>

      <section aria-labelledby="developers" className="space-y-6 border-t border-line pt-12 lg:pt-16">
        <h2 id="developers" className="text-[30px] leading-[38px] font-semibold lg:text-[40px] lg:leading-[48px]">
          {landing.devTitle}
        </h2>
        <p className="max-w-[1000px] text-lg leading-8 text-gray lg:text-xl lg:leading-8">
          {landing.devBody}{" "}
          <Link href={localized(lang, "/docs")} className="font-medium text-ink underline">
            {landing.readDocs} →
          </Link>
        </p>
      </section>
    </main>
  );
}
