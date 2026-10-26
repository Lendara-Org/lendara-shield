import { notFound } from "next/navigation";

import { type Messages, messages } from "@/content/messages";
import { type Lang, isLang } from "@/lib/i18n";

export type LangParams = { params: Promise<{ lang: string }> };

/** The page's language and copy; 404 for an unknown language. */
export async function pageCopy({ params }: LangParams): Promise<{ lang: Lang; t: Messages }> {
  const { lang } = await params;
  if (!isLang(lang)) notFound();
  return { lang, t: messages[lang] };
}
