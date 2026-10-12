import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, type RenderResult } from "@testing-library/react";
import { Suspense, type ReactNode } from "react";
import { I18nextProvider } from "react-i18next";

import { createI18n, loadersFromGlob, type Locale, type Messages } from "~/lib/i18n";

import en from "../src/app/i18n/en.json";

const moduleMessages = import.meta.glob<Messages>("../src/modules/*/i18n/*.json", {
  import: "default",
});

/** Renders `ui` with a fresh query cache (no retries) and real, lazily loaded i18n. */
export function renderWithProviders(ui: ReactNode, locale: Locale = "en"): RenderResult {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const i18n = createI18n({
    locale,
    shell: { en, pt: en, es: en, fr: en },
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
