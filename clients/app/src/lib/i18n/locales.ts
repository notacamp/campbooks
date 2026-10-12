export const SUPPORTED_LOCALES = ["en", "pt", "es", "fr"] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

function isLocale(value: string): value is Locale {
  return (SUPPORTED_LOCALES as readonly string[]).includes(value);
}

/**
 * Picks the UI locale: an explicit saved choice wins, then the first browser
 * language whose primary subtag we ship (`pt-PT` → `pt`), then English.
 */
export function detectLocale(saved: string | null, browserLanguages: readonly string[]): Locale {
  if (saved !== null && isLocale(saved)) {
    return saved;
  }
  for (const tag of browserLanguages) {
    const primary = tag.toLowerCase().split("-", 1)[0] ?? "";
    if (isLocale(primary)) {
      return primary;
    }
  }
  return DEFAULT_LOCALE;
}
