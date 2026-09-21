/**
 * modules/calendar/api — data hook for the Calendar surface.
 *
 * Wraps TanStack Query around GET /api/app/time.
 * Returns the full TimeAgenda shape from Time::Agenda serializer.
 */

import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { TimeAgenda } from "~/modules/calendar/components";

// ── Query key factory ─────────────────────────────────────────────────────────

export const calendarKeys = {
  all: ["calendar"] as const,
};

// ── Query hook ────────────────────────────────────────────────────────────────

/**
 * Fetches the Calendar/Time surface payload from GET /api/app/time.
 *
 * Response shape (unwrapped by apiClient):
 *   TimeAgenda — { items, undated, day_note, suggestions }
 *
 * staleTime: 60 s — events are synced by the minute poll cadence.
 */
export const useCalendarQuery = (): UseQueryResult<TimeAgenda, Error> =>
  useQuery<TimeAgenda, Error>({
    queryKey: calendarKeys.all,
    queryFn: () => apiClient<TimeAgenda>("/time"),
    staleTime: 60_000,
  });
