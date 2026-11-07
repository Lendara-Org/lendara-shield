import { headers } from "next/headers";
import { redirect } from "next/navigation";

import { localized, preferredLang } from "@/lib/i18n";

// / → /es or /en, from the browser's preferred language.
export default async function Root() {
  redirect(localized(preferredLang((await headers()).get("accept-language"))));
}
