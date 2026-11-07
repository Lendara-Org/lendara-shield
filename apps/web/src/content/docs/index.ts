import type { DocChrome, DocPage, DocSlug } from "@/lib/docs";
import type { Lang } from "@/lib/i18n";

import * as en from "./en";
import * as es from "./es";

export const docs: Record<Lang, { chrome: DocChrome; pages: Record<DocSlug, DocPage> }> = { en, es };
