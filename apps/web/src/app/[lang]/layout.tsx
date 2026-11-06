import type { Metadata, Viewport } from "next";
import { notFound } from "next/navigation";

import "../globals.css";

import { I18nProvider } from "@/components/i18n";
import { SessionProvider } from "@/components/session";
import { SiteHeader } from "@/components/site-header";
import { messages } from "@/content/messages";
import { fontVariables } from "@/lib/fonts";
import { LANGS, isLang } from "@/lib/i18n";

type Props = { children: React.ReactNode; params: Promise<{ lang: string }> };

export const dynamicParams = false;

export const viewport: Viewport = {
  themeColor: "#FFFFFF",
};

export function generateStaticParams() {
  return LANGS.map((lang) => ({ lang }));
}

export async function generateMetadata({ params }: Omit<Props, "children">): Promise<Metadata> {
  const { lang } = await params;
  if (!isLang(lang)) return {};
  const { meta } = messages[lang];
  return {
    title: { default: meta.title, template: `%s · ${meta.title}` },
    description: meta.description,
    alternates: { languages: { en: "/en", es: "/es" } },
  };
}

export default async function LangLayout({ children, params }: Props) {
  const { lang } = await params;
  if (!isLang(lang)) notFound();
  // The root layout of every page: <html lang> must be the page's language
  // for screen readers, translation and hyphenation (not a wrapper's).
  return (
    <html lang={lang} className={fontVariables}>
      <body>
        <I18nProvider lang={lang}>
          <SessionProvider>
            <SiteHeader />
            {children}
            <footer className="mx-auto max-w-[1440px] border-t border-line px-5 py-8 text-sm sm:px-8 lg:px-12 text-gray">{messages[lang].footer}</footer>
          </SessionProvider>
        </I18nProvider>
      </body>
    </html>
  );
}
