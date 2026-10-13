import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, type RenderResult } from "@testing-library/react";
import { Suspense, type ReactNode } from "react";
import { I18nextProvider } from "react-i18next";

import { createI18n, loadersFromGlob, type Locale, type Messages } from "~/lib/i18n";
import { ShortcutProvider, ShortcutRegistry, DEFAULT_SHORTCUTS } from "~/lib/shortcuts";
import { UI_NAMESPACES } from "~/lib/ui";

import en from "../src/app/i18n/en.json";

const moduleMessages = import.meta.glob<Messages>("../src/modules/*/i18n/*.json", {
  import: "default",
});

/** Renders `ui` with a fresh query cache (no retries) and real, lazily loaded i18n. */
export function renderWithProviders(ui: ReactNode, locale: Locale = "en"): RenderResult {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const i18n = createI18n({
    locale,
    shell: { en, "pt-PT": en, "es-ES": en, fr: en },
    bundled: UI_NAMESPACES,
    loaders: loadersFromGlob(moduleMessages),
  });
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <Suspense fallback={<p>loading translations</p>}>{ui}</Suspense>
      </QueryClientProvider>
    </I18nextProvider>,
  );
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
    registry = new ShortcutRegistry(DEFAULT_SHORTCUTS),
  }: { locale?: Locale; registry?: ShortcutRegistry } = {},
): RenderResult & { registry: ShortcutRegistry } {
  const i18n = createI18n({
    locale,
    shell: { en, "pt-PT": en, "es-ES": en, fr: en },
    bundled: UI_NAMESPACES,
    loaders: {},
  });
  const result = render(
    <I18nextProvider i18n={i18n}>
      <ShortcutProvider registry={registry} platform="other">
        {ui}
      </ShortcutProvider>
    </I18nextProvider>,
  );
  return { ...result, registry };
}

/** Stubs `fetch` with JSON responses keyed by request path. */
export function stubApi(routes: Record<string, { status?: number; body: unknown }>): void {
  globalThis.fetch = (input: RequestInfo | URL) => {
    const url = new URL(input instanceof Request ? input.url : String(input));
    const route = routes[url.pathname];
    if (route === undefined) {
      return Promise.reject(new TypeError(`Network error: no stub for ${url.pathname}`));
    }
    return Promise.resolve(Response.json(route.body, { status: route.status ?? 200 }));
  };
}
