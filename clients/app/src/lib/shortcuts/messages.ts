import type { Locale, Messages } from "~/lib/i18n";

import en from "./i18n/en.json";
import es from "./i18n/es-ES.json";
import fr from "./i18n/fr.json";
import pt from "./i18n/pt-PT.json";

/** The `shortcuts` i18n namespace (shortcut labels and group names), bundled eagerly. */
export const shortcutMessages: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };

export type ShortcutMessages = typeof en;
