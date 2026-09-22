/**
 * useOAuthSignInUrlQuery — lazily fetches an OAuth sign-in authorize URL.
 *
 * GET /api/app/oauth/sign_in_url?provider=<provider>&return_to=<url>
 * → { data: { provider, authorize_url } }
 *
 * The query is disabled until `enabled` is set to true (on user click).
 * Once the URL is available, the caller navigates the browser to it.
 *
 * Query key: ["oauth", "signInUrl", provider]
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "../client";
import type { OAuthProvider } from "./use-oauth-providers";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface OAuthSignInUrlResponse {
  provider: OAuthProvider;
  authorize_url: string;
}

// ── Hook ──────────────────────────────────────────────────────────────────────

export const useOAuthSignInUrlQuery = (
  provider: OAuthProvider | null,
  { enabled = false }: { enabled?: boolean } = {},
): UseQueryResult<OAuthSignInUrlResponse, Error> => {
  // return_to is the current /login page absolute URL so the provider redirects
  // back here and we can pick up ?token or ?error from the search params.
  const returnTo = `${window.location.origin}/login`;

  return useQuery<OAuthSignInUrlResponse>({
    queryKey: ["oauth", "signInUrl", provider],
    queryFn: async () =>
      apiClient<OAuthSignInUrlResponse>(
        `/oauth/sign_in_url?${new URLSearchParams({
          provider: provider!,
          return_to: returnTo,
        })}`,
      ),
    enabled: enabled && provider !== null,
    // State token lasts 30 min on the server — cache 5 min to be safe.
    staleTime: 5 * 60_000,
    gcTime: 5 * 60_000,
    retry: false,
  });
};
