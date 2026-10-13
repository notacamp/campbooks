import { useQuery } from "@tanstack/react-query";

import { api, operationKey, unwrap, type Schema } from "~/lib/api";

export type Connection = Schema<"ConnectionView">;

/**
 * `connectors.list_connections`: the workspace's connections the caller can
 * see (their own; all of them for owners and admins), oldest first.
 */
export function useConnections(workspaceId: string) {
  return useQuery({
    queryKey: operationKey("connectors.list_connections", { workspace_id: workspaceId }),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/connections", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).connections,
  });
}
