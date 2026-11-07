import type { Metadata, Viewport } from "next";

import "./globals.css";

import { messages } from "@/content/messages";
import { fontVariables } from "@/lib/fonts";
import { LANGS } from "@/lib/i18n";

// 404 for URLs that match no route. Pages have one root layout per language
// (app/[lang]/layout.tsx), so this page renders its own <html> and shows
// both languages: the URL tells us nothing reliable about the visitor's.
export const metadata: Metadata = {
  title: `${messages.en.notFound.title} · ${messages.en.meta.title}`,
};

// Not inherited from app/[lang]/layout.tsx, which this page bypasses.
export const viewport: Viewport = {
  themeColor: "#FFFFFF",
};

export default function GlobalNotFound() {
  return (
    <html lang="en" className={fontVariables}>
      <body>
        <main className="mx-auto max-w-[768px] px-5 py-16 sm:px-8">
          {LANGS.map((lang) => {
            const t = messages[lang].notFound;
            return (
              <section key={lang} lang={lang} className="mb-10">
                <h1 className="text-[40px] leading-[46px]">{t.title}</h1>
                <p className="mt-3 text-gray">{t.body}</p>
                <a className="mt-4 inline-block underline" href={`/${lang}`}>
                  {t.home}
                </a>
              </section>
            );
          })}
        </main>
      </body>
    </html>
  );
}
