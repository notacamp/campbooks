import createClient from "openapi-fetch";

import type { paths } from "./schema.gen";
import { selectTransport, type Transport } from "./transport";

export function createApiClient(transport: Transport) {
  return createClient<paths>({ baseUrl: transport.baseUrl, fetch: transport.fetch });
}

export type ApiClient = ReturnType<typeof createApiClient>;

const transport = selectTransport();

/** The app's one API client. Typed from `openapi/openapi.json`; transport picked at startup. */
export const api: ApiClient = createApiClient(transport);

/**
 * The address the browser itself can open for an API `path` (a top-level
 * navigation through the BFF, its session cookie attached), or `undefined`
 * where there is none: the native client's API is reached only through its
 * bridge.
 */
export function browserUrl(path: string): string | undefined {
  return transport.kind === "web" ? `${transport.baseUrl}${path}` : undefined;
}
