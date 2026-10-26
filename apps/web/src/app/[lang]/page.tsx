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
    <main className="mx-auto max-w-4xl space-y-12 px-4 py-10">
      <section className="space-y-4">
        <p className="text-sm font-medium text-wine">{landing.eyebrow}</p>
        <h1 className="font-display text-6xl leading-none tracking-wide">{landing.title}</h1>
        <p className="max-w-2xl text-gray">{landing.intro}</p>
        <div className="flex flex-wrap gap-3">
          <Link
            href={localized(lang, "/onboard")}
            className="inline-block rounded-md bg-ink px-5 py-3 text-sm font-medium text-paper hover:bg-ink/85"
          >
            {landing.start}
          </Link>
          <Link
            href={localized(lang, "/docs")}
            className="inline-block rounded-md border border-line bg-surface px-5 py-3 text-sm font-medium hover:border-ink/40"
          >
            {landing.readDocs}
          </Link>
        </div>
      </section>

      <section aria-labelledby="modes" className="space-y-4">
        <h2 id="modes" className="text-lg font-semibold">
          {landing.modesTitle}
        </h2>
        <div className="grid gap-4 sm:grid-cols-2">
          {modes.map((mode) => (
            <Link
              key={mode.title}
              href={mode.href}
              className="rounded-lg border border-line bg-surface p-6 transition-colors hover:border-ink/40"
            >
              <h3 className="mb-2 font-semibold">{mode.title}</h3>
              <p className="text-sm text-gray">{mode.body}</p>
            </Link>
          ))}
        </div>
      </section>

      <section aria-labelledby="developers" className="space-y-2">
        <h2 id="developers" className="text-lg font-semibold">
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
