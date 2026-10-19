import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, type Schema } from "~/lib/api";

export type ItemView = Schema<"ItemView">;
export type Reason = Schema<"Reason">;

/** Open items, what left the list today, or who Scout waits on for the caller. */
export type ListState = "open" | "cleared" | "waiting";

// Keyed by operationId, so the change feed refetches them when an
// `attention.item` changes.
const keys = {
  all: (workspaceId: string) => operationKey("attention.list", { workspace_id: workspaceId }),
  list: (workspaceId: string, state: ListState) =>
    operationKey("attention.list", { workspace_id: workspaceId, state }),
};

/**
 * `attention.list`: what needs the caller (or, `cleared`, what left their
 * list today; `waiting`, who Scout waits on for them, soonest check first).
 */
export function useAttention(workspaceId: string, state: ListState = "open") {
  return useQuery({
    queryKey: keys.list(workspaceId, state),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/attention", {
          // Where the member's day starts: Done today and what is overdue.
          params: {
            path: { workspace_id: workspaceId },
            query: { state, utc_offset: -new Date().getTimezoneOffset() },
          },
          signal,
        }),
      ),
  });
}

/**
 * How many things need the caller (the Home badge); `undefined` until known.
 * "Worth a look" asks nothing of them, so it is not counted
 * (campbooks-cloud#282).
 */
export function useAttentionCount(workspaceId: string): number | undefined {
  const query = useAttention(workspaceId);
  return query.data?.items.filter((item) => item.reason !== "worth_a_look").length;
}

/** Refetch every attention list of the workspace. */
export function useRefreshAttention(workspaceId: string) {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: keys.all(workspaceId) });
  };
}

/** `attention.snooze` (with `until`) and `attention.unsnooze` (without). */
export function useSnooze(workspaceId: string) {
  const refresh = useRefreshAttention(workspaceId);
  return useMutation({
    mutationFn: async ({ item, until }: { item: string; until: Date | undefined }) => {
      const params = { path: { workspace_id: workspaceId, item_id: item } };
      return until === undefined
        ? unwrap(
            await api.POST("/api/v1/w/{workspace_id}/attention/{item_id}/unsnooze", { params }),
          )
        : unwrap(
            await api.POST("/api/v1/w/{workspace_id}/attention/{item_id}/snooze", {
              params,
              body: { until: until.toISOString() },
            }),
          );
    },
    onSettled: refresh,
  });
}
