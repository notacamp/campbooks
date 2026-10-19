import type { Decorator, Preview } from "@storybook/react-vite";
import type { i18n as I18n } from "i18next";
import { useLayoutEffect, type ReactNode } from "react";
import { I18nextProvider } from "react-i18next";

import "../src/app/styles.css";
import en from "../src/app/i18n/en.json";
import es from "../src/app/i18n/es-ES.json";
import fr from "../src/app/i18n/fr.json";
import pt from "../src/app/i18n/pt-PT.json";
import { MODULE_SHORTCUTS } from "../src/app/modules";
import { createI18n, SUPPORTED_LOCALES, type Locale, type Messages } from "../src/lib/i18n";
import { DEFAULT_SHORTCUTS, ShortcutProvider, ShortcutRegistry } from "../src/lib/shortcuts";
import {
  ACCENTS,
  applyTheme,
  DENSITIES,
  THEME_MODES,
  UI_NAMESPACES,
  type Accent,
  type Density,
  type ThemeMode,
  type ThemeSettings,
} from "../src/lib/ui";

import { LONGEST, longestMessages } from "./longest";

// Module namespaces are bundled eagerly here, so screen stories never suspend.
const moduleFiles = import.meta.glob<Messages>("../src/modules/*/i18n/*.json", {
  import: "default",
  eager: true,
});
const moduleNamespaces: Record<string, Record<Locale, Messages>> = {};
for (const [path, messages] of Object.entries(moduleFiles)) {
  const match = /modules\/([^/]+)\/i18n\/([^/]+)\.json$/u.exec(path);
  if (match !== null) {
    const [, namespace = "", locale = ""] = match;
    moduleNamespaces[namespace] = {
      ...moduleNamespaces[namespace],
      [locale]: messages,
    } as Record<Locale, Messages>;
  }
}

/** A shipped locale, or the fit check's pseudo-locale (`.storybook/longest.ts`). */
type StoryLocale = Locale | typeof LONGEST;

const shell: Record<Locale, Messages> = { en, "pt-PT": pt, "es-ES": es, fr };
const bundled: Record<string, Record<Locale, Messages>> = {
  ...UI_NAMESPACES,
  ...moduleNamespaces,
};

/**
 * The pseudo-locale renders as French (its dates, numbers and plural rules)
 * with every string replaced by its longest translation.
 */
const asLongest = (messages: Record<Locale, Messages>): Record<Locale, Messages> => ({
  ...messages,
  fr: longestMessages(messages),
});

const instances = new Map<StoryLocale, I18n>();

function i18nFor(locale: StoryLocale): I18n {
  let instance = instances.get(locale);
  if (instance === undefined) {
    instance =
      locale === LONGEST
        ? createI18n({
            locale: "fr",
            shell: asLongest(shell),
            bundled: Object.fromEntries(
              Object.entries(bundled).map(([namespace, messages]) => [
                namespace,
                asLongest(messages),
              ]),
            ),
            loaders: {},
          })
        : createI18n({ locale, shell, bundled, loaders: {} });
    instances.set(locale, instance);
  }
  return instance;
}

function ThemeFrame({ theme, accent, density, children }: ThemeSettings & { children: ReactNode }) {
  useLayoutEffect(() => {
    applyTheme({ theme, accent, density }, document.documentElement);
  }, [theme, accent, density]);
  return children;
}

/** Theme × accent × density from the toolbar, applied to <html> like the app does. */
const withTheme: Decorator = (Story, { globals }) => (
  <ThemeFrame
    theme={globals["theme"] as ThemeMode}
    accent={globals["accent"] as Accent}
    density={globals["density"] as Density}
  >
    <Story />
  </ThemeFrame>
);

function LocaleFrame({ locale, children }: { locale: StoryLocale; children: ReactNode }) {
  useLayoutEffect(() => {
    document.documentElement.lang = locale === LONGEST ? "fr" : locale;
  }, [locale]);
  return <I18nextProvider i18n={i18nFor(locale)}>{children}</I18nextProvider>;
}

/** The story in the toolbar's language, with the app's real catalogues. */
const withI18n: Decorator = (Story, { globals }) => (
  <LocaleFrame locale={globals["locale"] as StoryLocale}>
    <Story />
  </LocaleFrame>
);

/** The app's shortcuts (defaults plus every module's), as in providers.tsx. */
const shortcuts = new ShortcutRegistry([...DEFAULT_SHORTCUTS, ...MODULE_SHORTCUTS]);

/** Shortcut hints and handlers, as in the app (⌘ on Apple, Ctrl elsewhere). */
const withShortcuts: Decorator = (Story) => (
  <ShortcutProvider registry={shortcuts}>
    <Story />
  </ShortcutProvider>
);

const toolbar = (
  title: string,
  icon: "mirror" | "paintbrush" | "component" | "globe",
  values: readonly string[],
) => ({
  description: title,
  toolbar: { title, icon, items: [...values], dynamicTitle: true },
});

const preview: Preview = {
  globalTypes: {
    theme: toolbar("Theme", "mirror", THEME_MODES),
    accent: toolbar("Accent", "paintbrush", ACCENTS),
    density: toolbar("Density", "component", DENSITIES),
    // "longest": every string in its longest translation, as `pnpm test:fit` renders it.
    locale: toolbar("Locale", "globe", [...SUPPORTED_LOCALES, LONGEST]),
  },
  initialGlobals: {
    // `pnpm test-storybook` runs every story in light and again in dark.
    theme: import.meta.env["VITE_STORYBOOK_THEME"] === "dark" ? "dark" : "light",
    accent: "business",
    density: "comfortable",
    locale: "en",
  },
  decorators: [withShortcuts, withI18n, withTheme],
  parameters: {
    layout: "padded",
    // Every story is an accessibility test: axe violations fail `pnpm test-storybook`.
    a11y: { test: "error" },
    controls: { matchers: { color: /^$/ } },
  },
};

export default preview;
