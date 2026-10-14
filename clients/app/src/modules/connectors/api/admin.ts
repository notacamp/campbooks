import { queryOptions } from "@tanstack/react-query";

import { adminApi, unwrap, type AdminSchema } from "~/lib/api";

/** A connection as an administrator sees it (admin API, ADR 0015). */
export type AdminConnection = AdminSchema<"AdminConnection">;

/** `connectors.admin_list_connections`: a workspace's connections and how they sync. */
export const adminConnectionsQuery = (workspaceId: string) =>
  queryOptions({
    queryKey: ["admin", "connectors.admin_list_connections", workspaceId] as const,
    queryFn: async ({ signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/connectors/workspaces/{workspace_id}/connections", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).connections,
  });
