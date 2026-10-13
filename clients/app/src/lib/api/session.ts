import { queryOptions, useQuery } from "@tanstack/react-query";
import createClient from "openapi-fetch";

import { unwrap } from "./errors";
import type { components, paths } from "./schema.gen";

/** Who is signed in to the web app (`GET /bff/session`). Never holds a token. */
type Session = components["schemas"]["SessionInfo"];

/**
 * The web app's session, from the server's BFF. Web only: in the Tauri shell
 * the native side owns sign-in. A signed-out browser gets
 * `{ authenticated: false }`, not an error.
 */
const sessionQuery = (origin: string = globalThis.location.origin) =>
  queryOptions({
    queryKey: ["bff", "session"],
    queryFn: async ({ signal }): Promise<Session> => {
      const bff = createClient<paths>({ baseUrl: origin });
      return unwrap(await bff.GET("/bff/session", { signal, credentials: "same-origin" }));
    },
    staleTime: 60_000,
  });

/** The current web session (TanStack Query over `GET /bff/session`). */
export function useSession() {
  return useQuery(sessionQuery());
}
