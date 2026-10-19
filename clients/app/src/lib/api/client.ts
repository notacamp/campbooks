import createClient from "openapi-fetch";

import type { paths as adminPaths } from "./admin.gen";
import type { paths } from "./schema.gen";
import { selectQuietTransport, selectTransport, type Transport } from "./transport";

export function createApiClient(transport: Transport) {
  return createClient<paths>({ baseUrl: transport.baseUrl, fetch: transport.fetch });
}

export type ApiClient = ReturnType<typeof createApiClient>;

/**
 * A client of the admin API (`/admin/v1/...`, ADR 0015), typed from
 * `openapi/admin.json`. It travels like the API: on the web through the
 * BFF (`/bff/admin/v1/...`), natively through the Rust bridge.
 */
export function createAdminClient(transport: Transport) {
  return createClient<adminPaths>({ baseUrl: transport.baseUrl, fetch: transport.fetch });
}

export type AdminClient = ReturnType<typeof createAdminClient>;

const transport = selectTransport();

/** The app's one API client. Typed from `openapi/openapi.json`; transport picked at startup. */
export const api: ApiClient = createApiClient(transport);

/**
 * Whether a response can be read as it arrives: on the web, through the
 * BFF. The native bridge hands back whole answers, so the desktop app asks
 * the JSON form of a streamed operation instead.
 */
export const canStream = transport.kind === "web";

/**
 * The address the browser itself can open for an API `path` (a top-level
 * navigation through the BFF, its session cookie attached), or `undefined`
 * where there is none: the native client's API is reached only through its
 * bridge.
 */
export function browserUrl(path: string): string | undefined {
  return transport.kind === "web" ? `${transport.baseUrl}${path}` : undefined;
}

/**
 * An API client for background calls nobody waits on (the app's own
 * telemetry): an ended session never sends the browser to sign in.
 */
export const backgroundApi: ApiClient = createApiClient(selectQuietTransport());

/** The app's admin API client, over the same transport. */
export const adminApi: AdminClient = createAdminClient(transport);
