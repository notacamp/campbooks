import type { Locale, Messages } from "./locales";
import { shortcutMessages } from "./shortcuts/messages";

import en from "./i18n/en.json";
import es from "./i18n/es-ES.json";
import fr from "./i18n/fr.json";
import pt from "./i18n/pt-PT.json";

/** The `ui` i18n namespace: strings the components own ("Undo", "Why?", ...). */
const uiMessages: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };

export type UiMessages = typeof en;

/**
 * Namespaces the design system needs on first render, per locale. Give them
 * to i18next as bundled resources so components never suspend on their own
 * labels. An app that adds shortcut labels merges them into `shortcuts`.
 */
export const UI_NAMESPACES = { ui: uiMessages, shortcuts: shortcutMessages } as const;
