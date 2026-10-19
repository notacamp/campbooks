import type { Locale, Messages } from "../src/lib/i18n";

/**
 * Wraps every string of the "longest" pseudo-locale. U+2060 (word joiner) is
 * invisible and takes no space, so layout is the real one, and the fit check
 * (`pnpm test:fit`) can tell our copy from demo content in the page.
 */
export const FIT_MARK = "⁠";

/** The toolbar's pseudo-locale: the longest translation of every string. */
export const LONGEST = "longest";

/** The translations a pseudo-locale string is picked from (English is the source, never the longest). */
const TRANSLATED: readonly Locale[] = ["pt-PT", "es-ES", "fr"];

const isMessages = (value: unknown): value is Messages =>
  typeof value === "object" && value !== null && !Array.isArray(value);

/** Key by key, the longer string of the two (nested keys merged by path). */
function longer(a: Messages, b: Messages): Messages {
  const merged: Messages = { ...a };
  for (const [key, value] of Object.entries(b)) {
    const current = merged[key];
    if (isMessages(value)) {
      merged[key] = longer(isMessages(current) ? current : {}, value);
    } else if (
      typeof value === "string" &&
      (typeof current !== "string" || value.length > current.length)
    ) {
      merged[key] = value;
    }
  }
  return merged;
}

function marked(messages: Messages): Messages {
  return Object.fromEntries(
    Object.entries(messages).map(([key, value]) => {
      if (typeof value === "string") {
        return [key, `${FIT_MARK}${value}${FIT_MARK}`];
      }
      return [key, isMessages(value) ? marked(value) : value];
    }),
  );
}

/**
 * One namespace in the pseudo-locale: for every key, the longest of the
 * Portuguese, Spanish and French strings, marked with FIT_MARK. Plural keys
 * are merged too (the three languages share one/many/other).
 */
export function longestMessages(byLocale: Record<Locale, Messages>): Messages {
  return marked(TRANSLATED.map((locale) => byLocale[locale]).reduce(longer, {}));
}
