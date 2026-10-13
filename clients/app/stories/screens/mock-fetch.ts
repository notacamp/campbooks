import { writePref } from "~/lib/storage/prefs";
import { DEFAULT_WORKSPACE } from "~/lib/ui/theme/theme";

/** A mocked endpoint's answer: a fixed response, or one computed from the request body. */
export type MockRoute =
  { status?: number; body?: unknown } | ((body: unknown) => { status?: number; body?: unknown });

/**
 * Replaces `fetch` with in-memory answers keyed by "METHOD /path" (stories
 * only: demo data, no server). Returns the function that restores it.
 */
export function mockFetch(routes: Record<string, MockRoute>): () => void {
  const original = globalThis.fetch;
  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const route = `${request.method} ${new URL(request.url).pathname}`;
    const text = await request.text();
    const handler = routes[route];
    if (handler === undefined) {
      return Response.json({ status: 404, code: "not_found", title: "Not found" }, { status: 404 });
    }
    const answer =
      typeof handler === "function" ? handler(text === "" ? undefined : JSON.parse(text)) : handler;
    const status = answer.status ?? 200;
    return answer.body === undefined
      ? new Response(null, { status })
      : Response.json(answer.body, { status });
  };
  return () => {
    globalThis.fetch = original;
  };
}

/**
 * The app's own ThemeProvider reads the device's saved theme, so a screen
 * story saves the toolbar's theme, accent and density first: the screen then
 * renders in them instead of in whatever the last story left behind.
 */
export function saveThemeFromGlobals(globals: Record<string, unknown>): void {
  const text = (value: unknown, fallback: string) => (typeof value === "string" ? value : fallback);
  writePref("workspace", DEFAULT_WORKSPACE);
  writePref("theme", text(globals["theme"], "light"));
  writePref(`accent.${DEFAULT_WORKSPACE}`, text(globals["accent"], "business"));
  writePref("density", text(globals["density"], "comfortable"));
}
