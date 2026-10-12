/**
 * Locales the UI ships, as BCP 47 tags. Region is explicit where it matters:
 * a bare `pt` means Brazilian Portuguese to `Intl` and CLDR, and we write
 * European Portuguese and Spanish from Spain.
 */
export const SUPPORTED_LOCALES = ["en", "pt-PT", "es-ES", "fr"] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

function isLocale(value: string): value is Locale {
  return (SUPPORTED_LOCALES as readonly string[]).includes(value);
}

function primarySubtag(tag: string): string {
  return tag.toLowerCase().split("-", 1)[0] ?? "";
}

/**
 * Picks the UI locale: an explicit saved choice wins, then the first browser
 * language whose primary subtag we ship (`pt-BR` → `pt-PT`, `es-MX` →
 * `es-ES`: the closest translation beats English), then English.
 */
export function detectLocale(saved: string | null, browserLanguages: readonly string[]): Locale {
  if (saved !== null && isLocale(saved)) {
    return saved;
  }
  for (const tag of browserLanguages) {
    const match = SUPPORTED_LOCALES.find((locale) => primarySubtag(locale) === primarySubtag(tag));
    if (match !== undefined) {
      return match;
    }
  }
  return DEFAULT_LOCALE;
}
