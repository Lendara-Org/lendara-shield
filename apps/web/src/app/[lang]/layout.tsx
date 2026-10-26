import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { I18nProvider } from "@/components/i18n";
import { SessionProvider } from "@/components/session";
import { SiteHeader } from "@/components/site-header";
import { messages } from "@/content/messages";
import { LANGS, isLang } from "@/lib/i18n";

type Props = { children: React.ReactNode; params: Promise<{ lang: string }> };

export const dynamicParams = false;

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
  return (
    <div lang={lang}>
      <I18nProvider lang={lang}>
        <SessionProvider>
          <SiteHeader />
          {children}
          <footer className="mx-auto max-w-[1440px] border-t border-line px-5 py-8 text-sm sm:px-8 lg:px-12 text-gray">{messages[lang].footer}</footer>
        </SessionProvider>
      </I18nProvider>
    </div>
  );
}
