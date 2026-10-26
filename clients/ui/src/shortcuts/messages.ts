import type { Locale, Messages } from "../locales";

import en from "./i18n/en.json";
import es from "./i18n/es-ES.json";
import fr from "./i18n/fr.json";
import pt from "./i18n/pt-PT.json";

/**
 * The design system's part of the `shortcuts` i18n namespace: the group names
 * and the labels of {@link UI_SHORTCUTS}. An app adds its own labels to the
 * same namespace (`{ ...shortcutMessages[locale], ...its own }`).
 */
export const shortcutMessages: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };

export type ShortcutMessages = typeof en;
