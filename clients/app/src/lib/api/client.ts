import createClient from "openapi-fetch";

import type { paths as adminPaths } from "./admin.gen";
import type { paths } from "./schema.gen";
import { selectTransport, type Transport } from "./transport";

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

/** The app's admin API client, over the same transport. */
export const adminApi: AdminClient = createAdminClient(transport);
