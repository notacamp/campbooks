import { queryOptions, useQuery } from "@tanstack/react-query";

import { unwrap } from "./errors";
import type { components } from "./schema.gen";
import { webRoutes } from "./web";

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
    queryFn: async ({ signal }): Promise<Session> =>
      unwrap(await webRoutes(origin).GET("/bff/session", { signal })),
    staleTime: 60_000,
  });

/** The current web session (TanStack Query over `GET /bff/session`). */
export function useSession() {
  return useQuery(sessionQuery());
}

/**
 * Signs out of the web app (`POST /bff/logout`): the BFF ends the session and
 * clears its cookie. Resolves to the identity provider's end-session URL to
 * navigate to next, or null when the provider has none.
 */
export async function signOut(origin: string = globalThis.location.origin): Promise<string | null> {
  const result = await webRoutes(origin).POST("/bff/logout", {
    // The BFF refuses unsafe requests without its CSRF header.
    params: { header: { "x-campbooks-csrf": "1" } },
  });
  return unwrap(result).logout_url ?? null;
}

/**
 * Signs out and leaves: to the identity provider's end-session page when it
 * has one (it comes back to the app), else to the app's root.
 */
export async function signOutAndLeave(
  leave: (url: string) => void = (url) => {
    globalThis.location.assign(url);
  },
): Promise<void> {
  const logoutUrl = await signOut();
  leave(logoutUrl ?? "/");
}
