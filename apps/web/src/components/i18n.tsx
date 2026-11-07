"use client";

/**
 * The page language for client components. Dictionaries are imported here
 * (not passed from the server) because they contain functions.
 */
import { createContext, useContext } from "react";

import { type Messages, messages } from "@/content/messages";
import { DEFAULT_LANG, type Lang, localized } from "@/lib/i18n";

const LangContext = createContext<Lang>(DEFAULT_LANG);

export function I18nProvider({ lang, children }: { lang: Lang; children: React.ReactNode }) {
  return <LangContext.Provider value={lang}>{children}</LangContext.Provider>;
}

export function useI18n(): { lang: Lang; t: Messages; href: (path: string) => string } {
  const lang = useContext(LangContext);
  return { lang, t: messages[lang], href: (path) => localized(lang, path) };
}
