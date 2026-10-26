import {
  shortcutMessages as designSystemMessages,
  type ShortcutMessages as DesignSystemMessages,
} from "@notacamp/ui/shortcuts";

import type { Locale, Messages } from "~/lib/i18n";

import en from "./i18n/en.json";
import es from "./i18n/es-ES.json";
import fr from "./i18n/fr.json";
import pt from "./i18n/pt-PT.json";

/** The labels of Campbooks' own shortcuts (catalog.ts). */
const appMessages: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };

/**
 * The `shortcuts` i18n namespace (shortcut labels and group names), bundled
 * eagerly: the design system's labels and groups, then Campbooks' labels.
 */
export const shortcutMessages: Record<Locale, Messages> = {
  en: { ...designSystemMessages.en, ...appMessages.en },
  "pt-PT": { ...designSystemMessages["pt-PT"], ...appMessages["pt-PT"] },
  "es-ES": { ...designSystemMessages["es-ES"], ...appMessages["es-ES"] },
  fr: { ...designSystemMessages.fr, ...appMessages.fr },
};

export type ShortcutMessages = DesignSystemMessages & typeof en;
