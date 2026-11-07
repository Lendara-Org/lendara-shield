import type { Lang } from "@/lib/i18n";

import { type Messages, en } from "./en";
import { es } from "./es";

export type { Messages };

export const messages: Record<Lang, Messages> = { en, es };
