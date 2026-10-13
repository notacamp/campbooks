import createClient from "openapi-fetch";

import type { paths } from "./schema.gen";

/**
 * A typed client for the server's own web routes on this origin: the BFF
 * (`/bff/*`) and the sign-in screens' routes (`/auth/*`). They answer to the
 * browser's cookies, never to a token. Built per call so it always uses the
 * current `fetch`.
 */
export function webRoutes(origin: string = globalThis.location.origin) {
  return createClient<paths>({
    baseUrl: origin,
    credentials: "same-origin",
    fetch: (request) => globalThis.fetch(request),
  });
}
