import {
  infiniteQueryOptions,
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { api, operationKey, unwrap, type Schema } from "~/lib/api";

export type SearchHit = Schema<"Hit">;
export type SearchFilter = Schema<"AppliedFilter">;
export type SearchWarning = Schema<"QueryWarning">;

/** The order of a page of results. */
export type SearchSort = "relevance" | "newest";

/** Rows in the overlay: a glance, not a list. */
const INSTANT_ROWS = 8;
/** Rows per page on the results page. */
const PAGE = 20;

/** The member's zone, for `before:` and `after:`. */
function zone(): { tz?: string } {
  try {
    return { tz: Intl.DateTimeFormat().resolvedOptions().timeZone };
  } catch {
    return {};
  }
}

// Keyed by operationId (`operationKey`), so the change feed refetches them.
const key = (workspaceId: string, params: Record<string, string>) =>
  operationKey("search.query", { workspace_id: workspaceId, ...params });

/**
 * `search.query` for the overlay: the best rows by relevance, the last word
 * matched as a prefix (the member is still typing). The previous rows stay
 * until the next arrive; a superseded request is aborted through its signal.
 */
export function useInstantSearch(workspaceId: string, text: string) {
  const q = text.trim();
  return useQuery({
    queryKey: key(workspaceId, {
      q,
      sort: "relevance",
      prefix: "true",
      limit: String(INSTANT_ROWS),
    }),
    enabled: q !== "",
    placeholderData: keepPreviousData,
    staleTime: 10_000,
    retry: false,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/search", {
          params: {
            path: { workspace_id: workspaceId },
            query: {
              q,
              sort: "relevance",
              prefix: true,
              limit: INSTANT_ROWS,
              ...zone(),
            },
          },
          signal,
        }),
      ),
  });
}

const resultsQuery = (workspaceId: string, text: string, sort: SearchSort) => {
  const q = text.trim();
  return infiniteQueryOptions({
    queryKey: key(workspaceId, { q, sort, limit: String(PAGE) }),
    initialPageParam: undefined as string | undefined,
    enabled: q !== "",
    placeholderData: keepPreviousData,
    retry: false,
    queryFn: async ({ pageParam, signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/search", {
          params: {
            path: { workspace_id: workspaceId },
            query: {
              q,
              sort,
              limit: PAGE,
              ...zone(),
              // Opaque: handed back exactly as the server gave it.
              ...(pageParam === undefined ? {} : { cursor: pageParam }),
            },
          },
          signal,
        }),
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
};

/** `search.query` for the results page, a page at a time (`next_cursor` is opaque). */
export function useSearchResults(workspaceId: string, text: string, sort: SearchSort) {
  return useInfiniteQuery(resultsQuery(workspaceId, text, sort));
}

/**
 * Starts searching again from the first page: forgets every cached page of
 * the workspace's searches. For a cursor that no longer fits (409-style
 * `cursor_mismatch`) and for plain retries.
 */
export function useSearchRestart(workspaceId: string): () => void {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.resetQueries({
      queryKey: operationKey("search.query", { workspace_id: workspaceId }),
    });
  };
}
