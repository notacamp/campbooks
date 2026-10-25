import {
  UI_NAMESPACES as DESIGN_SYSTEM_NAMESPACES,
  type UiMessages as DesignSystemMessages,
} from "@notacamp/ui";

import type { Locale, Messages } from "~/lib/i18n";
import { shortcutMessages } from "~/lib/shortcuts";

import en from "./i18n/en.json";
import es from "./i18n/es-ES.json";
import fr from "./i18n/fr.json";
import pt from "./i18n/pt-PT.json";

/** The strings of the components that stay in the app, in the same `ui` namespace. */
const appMessages: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };

const uiMessages: Record<Locale, Messages> = {
  en: { ...DESIGN_SYSTEM_NAMESPACES.ui.en, ...appMessages.en },
  "pt-PT": { ...DESIGN_SYSTEM_NAMESPACES.ui["pt-PT"], ...appMessages["pt-PT"] },
  "es-ES": { ...DESIGN_SYSTEM_NAMESPACES.ui["es-ES"], ...appMessages["es-ES"] },
  fr: { ...DESIGN_SYSTEM_NAMESPACES.ui.fr, ...appMessages.fr },
};

/** The `ui` i18n namespace: strings the components own ("Undo", "Why?", ...). */
export type UiMessages = DesignSystemMessages & typeof en;

/**
 * Namespaces the design system needs on first render. Pass them to
 * `createI18n({ bundled })` so components never suspend on their own labels.
 */
export const UI_NAMESPACES = { ui: uiMessages, shortcuts: shortcutMessages } as const;
