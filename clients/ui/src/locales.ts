/**
 * The locales the design system's own strings ship in, as full BCP 47 tags
 * (a bare `pt` means Brazilian Portuguese to `Intl`): English, European
 * Portuguese, Spanish from Spain and French.
 */
export const UI_LOCALES = ["en", "pt-PT", "es-ES", "fr"] as const;
export type Locale = (typeof UI_LOCALES)[number];

/** One namespace's messages for one locale, as i18next takes them. */
export type Messages = Record<string, unknown>;
