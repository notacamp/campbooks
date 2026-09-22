/**
 * useOAuthProvidersQuery — fetches the list of enabled OAuth sign-in providers.
 *
 * GET /api/app/oauth/providers (UNAUTHENTICATED)
 * → { data: { providers: ["google", "zoho"] } }   (includes "microsoft" only when enabled)
 *
 * This query intentionally does NOT require a bearer token — it is called on
 * the login page before any session exists.
 *
 * Query key: ["oauth", "providers"]
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "../client";

// ── Types ─────────────────────────────────────────────────────────────────────

export type OAuthProvider = "google" | "zoho" | "microsoft";

export interface OAuthProvidersResponse {
  providers: OAuthProvider[];
}

// ── Hook ──────────────────────────────────────────────────────────────────────

export const useOAuthProvidersQuery = (): UseQueryResult<
  OAuthProvidersResponse,
  Error
> =>
  useQuery<OAuthProvidersResponse>({
    queryKey: ["oauth", "providers"],
    queryFn: () => apiClient<OAuthProvidersResponse>("/oauth/providers"),
    // Providers list rarely changes — cache for 5 minutes.
    staleTime: 5 * 60_000,
    // Never retry: if the endpoint is down, fall back silently (no buttons shown).
    retry: false,
  });
