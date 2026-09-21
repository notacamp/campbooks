/**
 * modules/settings/api/use-account-query — GET /api/app/account
 *
 * Wraps TanStack Query around the account profile endpoint.
 * Returns the current user's account data and preferences.
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { AccountData } from "~/modules/settings/types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const accountKeys = {
  all: ["settings", "account"] as const,
};

// ── Query hook ────────────────────────────────────────────────────────────────

/**
 * Fetches the current user's account profile from GET /api/app/account.
 * staleTime: 5 min — settings data changes infrequently.
 */
export const useAccountQuery = (): UseQueryResult<AccountData, Error> =>
  useQuery<AccountData, Error>({
    queryKey: accountKeys.all,
    queryFn: () => apiClient<AccountData>("/account"),
    staleTime: 5 * 60_000,
  });
