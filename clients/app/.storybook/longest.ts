import type { Locale, Messages } from "../src/lib/i18n";

/**
 * Wraps every string of the "longest" pseudo-locale. U+2060 (word joiner) is
 * invisible and takes no space, so layout is the real one, and the fit check
 * (`pnpm test:fit`) can tell our copy from demo content in the page.
 */
export const FIT_MARK = "⁠";

/** The toolbar's pseudo-locale: the widest translation of every string. */
export const LONGEST = "longest";

/** Every shipped language: English is usually the shortest, not always. */
const TRANSLATED: readonly Locale[] = ["en", "pt-PT", "es-ES", "fr"];

const isMessages = (value: unknown): value is Messages =>
  typeof value === "object" && value !== null && !Array.isArray(value);

// Strings are compared by how wide they render, not by how many characters
// they have ("Chercher ou demander" is as wide as "Pesquise ou pergunte").
// The app's font may not have loaded yet; its fallback ranks widths alike.
const canvas = typeof document === "undefined" ? null : document.createElement("canvas");
const context = canvas?.getContext("2d") ?? null;
if (context !== null) {
  context.font = "14px 'Funnel Sans Variable', ui-sans-serif, system-ui, sans-serif";
}
const width = (text: string) => context?.measureText(text).width ?? text.length;

/** Key by key, the wider string of the two (nested keys merged by path). */
function longer(a: Messages, b: Messages): Messages {
  const merged: Messages = { ...a };
  for (const [key, value] of Object.entries(b)) {
    const current = merged[key];
    if (isMessages(value)) {
      merged[key] = longer(isMessages(current) ? current : {}, value);
    } else if (
      typeof value === "string" &&
      (typeof current !== "string" || width(value) > width(current))
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
 * One namespace in the pseudo-locale: for every key, the longest of its
 * English, Portuguese, Spanish and French strings, marked with FIT_MARK.
 * Plural keys are merged too (a key only English has, `_one`, still counts).
 */
export function longestMessages(byLocale: Record<Locale, Messages>): Messages {
  return marked(TRANSLATED.map((locale) => byLocale[locale]).reduce(longer, {}));
}
