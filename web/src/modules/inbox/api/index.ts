/**
 * modules/inbox/api — data hooks for the Inbox module.
 *
 * Wraps TanStack Query around the /api/app/people endpoints.
 * Exports:
 *   useInboxInfiniteQuery(tab, q) — infinite page-based list
 *   useArchiveMutation()          — archive / unarchive with optimistic update
 */

import {
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  type UseInfiniteQueryResult,
  type UseMutationResult,
  type InfiniteData,
} from "@tanstack/react-query";
import { apiCollection, apiClient } from "~/lib/api";
import type { InboxRow, InboxTab, InboxQueryData, ArchiveAction } from "../types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const inboxKeys = {
  all: ["inbox"] as const,
  list: (tab: InboxTab, q: string) => ["inbox", tab, q] as const,
};

// ── Types ─────────────────────────────────────────────────────────────────────

interface PageResult {
  data: InboxRow[];
  meta: { page: number; per_page: number; total: number; total_pages: number };
}

// ── List hook ─────────────────────────────────────────────────────────────────

/**
 * Infinite-paged list of inbox rows.
 *
 * @param tab  "needing" → Important (needs_you=true); "latest" → Everything.
 * @param q    Optional search query.
 */
export const useInboxInfiniteQuery = (
  tab: InboxTab,
  q: string,
): UseInfiniteQueryResult<InfiniteData<PageResult>, Error> =>
  useInfiniteQuery<PageResult, Error, InfiniteData<PageResult>, ReturnType<typeof inboxKeys.list>, number>({
    queryKey: inboxKeys.list(tab, q),
    queryFn: ({ pageParam }) => {
      const params = new URLSearchParams({ tab, page: String(pageParam) });
      if (q) params.set("q", q);
      return apiCollection<InboxRow>(`/people?${params.toString()}`);
    },
    getNextPageParam: (lastPage) => {
      const { page, total_pages } = lastPage.meta;
      return page < total_pages ? page + 1 : undefined;
    },
    initialPageParam: 1,
    staleTime: 20_000,
  });

// ── Archive mutation ───────────────────────────────────────────────────────────

/**
 * Archive / unarchive a person row via POST /api/app/people/:id/action.
 *
 * onMutate: optimistic removal from the cache (all tabs/pages).
 * onError: restores the snapshot if the server rejects.
 * onSettled: invalidates the query so the server state is the final truth.
 *
 * Undo flow: call `mutate({ id, kind: "unarchive" })` — the optimistic
 * remove is skipped when kind=="unarchive" (the row is already gone),
 * and the settled invalidation brings it back from the server.
 */
export const useArchiveMutation = (): UseMutationResult<
  void,
  Error,
  { id: number; kind: ArchiveAction["kind"] },
  { snapshots: Map<string, InboxQueryData> }
> => {
  const queryClient = useQueryClient();

  return useMutation<
    void,
    Error,
    { id: number; kind: ArchiveAction["kind"] },
    { snapshots: Map<string, InboxQueryData> }
  >({
    mutationFn: async ({ id, kind }) => {
      await apiClient<void>(`/people/${id}/action`, {
        method: "POST",
        body: { kind } as unknown as undefined,
      });
    },

    onMutate: async ({ id, kind }) => {
      // Only optimistically remove on archive (not restore).
      if (kind !== "archive") return { snapshots: new Map() };

      // Cancel any in-flight fetches so they don't overwrite our optimistic update.
      await queryClient.cancelQueries({ queryKey: inboxKeys.all });

      // Snapshot all current inbox query data for rollback.
      const snapshots = new Map<string, InboxQueryData>();
      const cache = queryClient.getQueriesData<InboxQueryData>({
        queryKey: inboxKeys.all,
      });
      for (const [key, data] of cache) {
        if (data) snapshots.set(JSON.stringify(key), data);
      }

      // Optimistically remove the row from all cached pages.
      queryClient.setQueriesData<InboxQueryData>(
        { queryKey: inboxKeys.all },
        (old) => {
          if (!old) return old;
          return {
            ...old,
            pages: old.pages.map((page) => ({
              ...page,
              data: page.data.filter((row) => row.id !== id),
            })),
          };
        },
      );

      return { snapshots };
    },

    onError: (_err, _vars, context) => {
      if (!context?.snapshots) return;
      // Roll back all snapshots.
      for (const [keyStr, data] of context.snapshots) {
        const key = JSON.parse(keyStr) as unknown[];
        queryClient.setQueryData(key, data);
      }
    },

    onSettled: () => {
      // Always sync with server after settle.
      queryClient.invalidateQueries({ queryKey: inboxKeys.all });
    },
  });
};
