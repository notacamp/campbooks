import i18next, { type i18n } from "i18next";
import { initReactI18next } from "react-i18next";

import { lazyBackend, type Messages, type NamespaceLoader } from "./lazy-backend";
import { DEFAULT_LOCALE, SUPPORTED_LOCALES, type Locale } from "./locales";

export { detectLocale, type Locale } from "./locales";
export { loadersFromGlob, type Messages } from "./lazy-backend";

/** The app shell's namespace: bundled eagerly so the frame renders instantly. */
const SHELL_NAMESPACE = "shell";

export interface I18nSetup {
  locale: Locale;
  /** Shell messages for every supported locale (eager). */
  shell: Record<Locale, Messages>;
  /** Module namespaces, loaded on first use (lazy). */
  loaders: Record<string, NamespaceLoader>;
}

/**
 * Creates the i18next instance. Initialisation is synchronous because the
 * shell namespace is bundled; module namespaces load when `useTranslation(ns)`
 * first suspends on them.
 */
export function createI18n({ locale, shell, loaders }: I18nSetup): i18n {
  const instance = i18next.createInstance();
  void instance
    .use(lazyBackend(loaders))
    .use(initReactI18next)
    .init({
      lng: locale,
      fallbackLng: DEFAULT_LOCALE,
      supportedLngs: SUPPORTED_LOCALES,
      ns: [SHELL_NAMESPACE],
      defaultNS: SHELL_NAMESPACE,
      resources: Object.fromEntries(
        SUPPORTED_LOCALES.map((code) => [code, { [SHELL_NAMESPACE]: shell[code] }]),
      ),
      // Keep the bundled shell AND ask the backend for everything else.
      partialBundledLanguages: true,
      initAsync: false,
      // React already escapes rendered strings.
      interpolation: { escapeValue: false },
    });
  return instance;
}
