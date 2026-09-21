/**
 * modules/today/api — data hook for the Today surface.
 *
 * Wraps TanStack Query around GET /api/app/today.
 * Returns the full TodayData shape produced by the TodaySerializer.
 */

import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { TodayData } from "~/modules/today/components";

// ── Query key factory ─────────────────────────────────────────────────────────

export const todayKeys = {
  all: ["today"] as const,
};

// ── Query hook ────────────────────────────────────────────────────────────────

/**
 * Fetches the Today surface payload from GET /api/app/today.
 *
 * Response shape (wrapped in { data } envelope, unwrapped by apiClient):
 *   { greeting: { name, date, brief }, needs_you: NeedsYouItem[], coming_up: ComingUpItem[], handled: HandledCounts }
 *
 * staleTime: 60 s — the Today surface refreshes on the minute poll cadence.
 */
export const useTodayQuery = (): UseQueryResult<TodayData, Error> =>
  useQuery<TodayData, Error>({
    queryKey: todayKeys.all,
    queryFn: () => apiClient<TodayData>("/today"),
    staleTime: 60_000,
  });
