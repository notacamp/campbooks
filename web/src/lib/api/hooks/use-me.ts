/**
 * useMe — fetches the authenticated user from GET /api/app/me.
 *
 * Returns the current user's identity. On a 401 (no token / token expired),
 * the query reports an ApiError with status 401 — the calling route renders
 * "Not signed in". The real auth flow (exchanging the Rails session for a
 * bearer token) belongs to the auth module, not here.
 *
 * Query key: ["me"]
 * Staletime inherits the QueryClient default (30 s).
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "../client";

// ── Types (align with Rails API serializer when auth module lands) ────────────

export interface MeUser {
  id: number;
  name: string;
  email: string;
  role: string;
  locale: string;
  time_zone: string;
}

export interface MeWorkspace {
  id: number;
  name: string;
  app_name: string;
  self_hosted: boolean;
}

/** The /api/app/me bootstrap payload — matches Api::App::MeSerializer. */
export interface Me {
  user: MeUser;
  workspace: MeWorkspace;
  features: Record<string, boolean>;
}

// ── Hook ──────────────────────────────────────────────────────────────────────

export const useMe = (): UseQueryResult<Me> =>
  useQuery<Me>({
    queryKey: ["me"],
    queryFn: () => apiClient<Me>("/me"),
    // Never retry a 401 — the user is not signed in.
    retry: false,
  });
