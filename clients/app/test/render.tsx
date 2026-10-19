import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, type RenderResult } from "@testing-library/react";
import { Suspense, type ReactNode } from "react";
import { I18nextProvider } from "react-i18next";

import { createI18n, type Locale, type Messages } from "~/lib/i18n";
import { ShortcutProvider, ShortcutRegistry, DEFAULT_SHORTCUTS } from "~/lib/shortcuts";
import { UI_NAMESPACES, UndoProvider } from "~/lib/ui";

import { MODULE_SHORTCUTS } from "../src/app/modules";

import en from "../src/app/i18n/en.json";

// Module namespaces are bundled eagerly in tests: the app loads them lazily
// (src/lib/i18n covers that path), but a lazy load per test would only add
// seconds of Suspense to every screen test on a busy runner.
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

/** The app's shortcuts: the defaults plus every module's (as in providers.tsx). */
const appShortcuts = () => new ShortcutRegistry([...DEFAULT_SHORTCUTS, ...MODULE_SHORTCUTS]);

/**
 * Renders `ui` like the app does: a fresh query cache (no retries), real
 * i18n, the app's shortcuts (non-Apple keys) and the undo stack.
 */
export function renderWithProviders(
  ui: ReactNode,
  locale: Locale = "en",
): RenderResult & { queryClient: QueryClient } {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const i18n = createI18n({
    locale,
    shell: { en, "pt-PT": en, "es-ES": en, fr: en },
    bundled: { ...UI_NAMESPACES, ...moduleNamespaces },
    loaders: {},
  });
  const registry = appShortcuts();
  // Providers go in a wrapper so `rerender` keeps them.
  function Providers({ children }: { children: ReactNode }) {
    return (
      <I18nextProvider i18n={i18n}>
        <ShortcutProvider registry={registry} platform="other">
          <UndoProvider>
            <QueryClientProvider client={queryClient}>
              <Suspense fallback={<p>loading translations</p>}>{children}</Suspense>
            </QueryClientProvider>
          </UndoProvider>
        </ShortcutProvider>
      </I18nextProvider>
    );
  }
  const result = render(ui, { wrapper: Providers });
  return { ...result, queryClient };
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

/** What a mocked endpoint answers: a fixed response, or one computed from the request. */
interface MockAnswer {
  status?: number;
  body?: unknown;
  /** Server-Sent Events text, sent as one `text/event-stream` body in these chunks. */
  stream?: string[];
  /** Keep the stream open after its chunks, as a reply still being written. */
  open?: boolean;
}
export type MockHandler = MockAnswer | ((request: Request, body: unknown) => MockAnswer);

export interface MockedApi {
  /** Every request made, as "METHOD /path" with its JSON body. */
  calls: { route: string; body: unknown }[];
}

/**
 * Stubs `fetch` per method and path: keys are "GET /bff/api/v1/workspaces",
 * "PATCH /bff/api/v1/w/w1/members/u2", ... An unknown route is a network error.
 */
export function mockApi(routes: Record<string, MockHandler>): MockedApi {
  const mocked: MockedApi = { calls: [] };
  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const route = `${request.method} ${new URL(request.url).pathname}`;
    const text = await request.text();
    const body: unknown = text === "" ? undefined : JSON.parse(text);
    mocked.calls.push({ route, body });
    const handler = routes[route];
    if (handler === undefined) {
      throw new TypeError(`Network error: no mock for ${route}`);
    }
    const answer = typeof handler === "function" ? handler(request, body) : handler;
    const status = answer.status ?? 200;
    if (answer.stream !== undefined) {
      const encoder = new TextEncoder();
      const chunks = answer.stream;
      return new Response(
        new ReadableStream<Uint8Array>({
          start(controller) {
            for (const chunk of chunks) {
              controller.enqueue(encoder.encode(chunk));
            }
            if (answer.open !== true) {
              controller.close();
            }
          },
        }),
        { status, headers: { "content-type": "text/event-stream" } },
      );
    }
    return answer.body === undefined
      ? new Response(null, { status })
      : Response.json(answer.body, {
          status,
          headers: {
            "content-type": status >= 400 ? "application/problem+json" : "application/json",
          },
        });
  };
  return mocked;
}

/**
 * A streamed reply as `conversations.stream_reply` sends it: some thinking,
 * the words, then `done` with `text`.
 */
export function replyStream(
  messageId: string,
  text: string,
  thinking = "Reading the message.",
): MockAnswer {
  const frame = (event: string, data: unknown) =>
    `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
  return {
    stream: [
      frame("thinking", { delta: thinking }) +
        frame("text", { delta: text }) +
        frame("done", { message_id: messageId, text }),
    ],
  };
}

/** A problem+json body as the API sends it. */
export function problem(status: number, code: string): MockAnswer {
  return {
    status,
    body: { type: "about:blank", title: "Problem", status, detail: "Localized detail", code },
  };
}
