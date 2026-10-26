import { render, type RenderResult } from "@testing-library/react";
import i18next, { type i18n } from "i18next";
import type { ReactNode } from "react";
import { I18nextProvider, initReactI18next } from "react-i18next";

import { UI_LOCALES, type Locale, type Messages } from "../src/locales";
import { UI_NAMESPACES } from "../src/messages";
import { ShortcutProvider, ShortcutRegistry } from "../src/shortcuts";

import en from "./shortcut-labels/en.json";
import es from "./shortcut-labels/es-ES.json";
import fr from "./shortcut-labels/fr.json";
import pt from "./shortcut-labels/pt-PT.json";
import { TEST_SHORTCUTS } from "./shortcuts";

/** The labels of the test catalogue's app shortcuts, by locale. */
const appLabels: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };

/**
 * i18next with the design system's bundled namespaces, the `shortcuts` one
 * holding the app's labels too, as an app sets it up.
 */
function testI18n(locale: Locale): i18n {
  const instance = i18next.createInstance();
  void instance.use(initReactI18next).init({
    lng: locale,
    fallbackLng: "en",
    supportedLngs: UI_LOCALES,
    load: "currentOnly",
    ns: ["ui", "shortcuts"],
    defaultNS: "ui",
    resources: Object.fromEntries(
      UI_LOCALES.map((code) => [
        code,
        {
          ui: UI_NAMESPACES.ui[code],
          shortcuts: { ...UI_NAMESPACES.shortcuts[code], ...appLabels[code] },
        },
      ]),
    ),
    initAsync: false,
    interpolation: { escapeValue: false },
  });
  return instance;
}

/**
 * Renders design-system UI with real i18n (bundled `ui` and `shortcuts`
 * namespaces) and a fresh shortcut registry on the non-Apple platform, so
 * hints read "Ctrl" and key presses on `document` reach bound handlers.
 */
export function renderUi(
  ui: ReactNode,
  {
    locale = "en",
    registry = new ShortcutRegistry(TEST_SHORTCUTS),
  }: { locale?: Locale; registry?: ShortcutRegistry } = {},
): RenderResult & { registry: ShortcutRegistry } {
  const result = render(
    <I18nextProvider i18n={testI18n(locale)}>
      <ShortcutProvider registry={registry} platform="other">
        {ui}
      </ShortcutProvider>
    </I18nextProvider>,
  );
  return { ...result, registry };
}
