/**
 * lib/i18n — react-intl provider and message helpers.
 *
 * Key format: `module.feature.element` (e.g. "auth.signIn.title",
 * "people.inbox.title"). Form namespaces use the same dotted convention.
 *
 * Locales supported: en (default), pt, es, fr.
 *
 * Locale catalogs are loaded eagerly at boot — for the initial scaffold all
 * messages are in a single JSON file per locale. As the app grows, split into
 * per-module slices and load lazily with the route code.
 *
 * Usage in modules (via react-intl):
 *   import { useIntl, FormattedMessage } from "react-intl";
 *   const { formatMessage } = useIntl();
 *   formatMessage({ id: "people.inbox.title" })
 *   <FormattedMessage id="common.save" />
 */
import {
  IntlProvider as ReactIntlProvider,
  useIntl as useReactIntl,
  defineMessages,
  type IntlShape,
} from "react-intl";
import { type FC, type ReactNode } from "react";

// ── Locale catalog imports ────────────────────────────────────────────────────

import enMessages from "./locales/en.json";
import ptMessages from "./locales/pt.json";
import esMessages from "./locales/es.json";
import frMessages from "./locales/fr.json";

type SupportedLocale = "en" | "pt" | "es" | "fr";

const catalogs: Record<SupportedLocale, Record<string, string>> = {
  en: enMessages,
  pt: ptMessages,
  es: esMessages,
  fr: frMessages,
};

const SUPPORTED_LOCALES: SupportedLocale[] = ["en", "pt", "es", "fr"];
const DEFAULT_LOCALE: SupportedLocale = "en";

/** Resolves a browser Accept-Language string to a supported locale code. */
const resolveLocale = (requested: string): SupportedLocale => {
  const language = requested.split("-")[0].toLowerCase() as SupportedLocale;
  return SUPPORTED_LOCALES.includes(language) ? language : DEFAULT_LOCALE;
};

/** The active locale — resolved from navigator.language at boot. */
const activeLocale: SupportedLocale = resolveLocale(
  typeof navigator !== "undefined" ? navigator.language : DEFAULT_LOCALE,
);

// ── Provider ──────────────────────────────────────────────────────────────────

interface IntlProviderProps {
  children: ReactNode;
  /** Override locale (useful in tests or user-preference flows). */
  locale?: SupportedLocale;
}

export const IntlProvider: FC<IntlProviderProps> = ({
  children,
  locale = activeLocale,
}) => (
  <ReactIntlProvider
    locale={locale}
    messages={catalogs[locale]}
    defaultLocale={DEFAULT_LOCALE}
    onError={() => {
      // Swallow missing-translation warnings in production.
      // In development, react-intl logs them to the console via its own handler.
    }}
  >
    {children}
  </ReactIntlProvider>
);

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Typed re-export of useIntl for consistent imports. */
export const useIntl = (): IntlShape => useReactIntl();

/**
 * Defines a typed message descriptor set.
 * Used to declare a module's messages near the component that uses them:
 *   const messages = defineMessages({ ... });
 */
export { defineMessages };

export type { SupportedLocale };
export { DEFAULT_LOCALE, activeLocale };
