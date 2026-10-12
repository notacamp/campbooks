import createClient from "openapi-fetch";

import type { paths } from "./schema.gen";
import { selectTransport, type Transport } from "./transport";

export function createApiClient(transport: Transport) {
  return createClient<paths>({ baseUrl: transport.baseUrl, fetch: transport.fetch });
}

export type ApiClient = ReturnType<typeof createApiClient>;

/** The app's one API client. Typed from `openapi/openapi.json`; transport picked at startup. */
export const api: ApiClient = createApiClient(selectTransport());
