import type { Locale, Messages } from "~/lib/i18n";
import { shortcutMessages } from "~/lib/shortcuts";

import en from "./i18n/en.json";
import es from "./i18n/es-ES.json";
import fr from "./i18n/fr.json";
import pt from "./i18n/pt-PT.json";

/** The `ui` i18n namespace: strings the components own ("Undo", "Why?", ...). */
const uiMessages: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };

export type UiMessages = typeof en;

/**
 * Namespaces the design system needs on first render. Pass them to
 * `createI18n({ bundled })` so components never suspend on their own labels.
 */
export const UI_NAMESPACES = { ui: uiMessages, shortcuts: shortcutMessages } as const;
