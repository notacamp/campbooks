import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, unwrapEmpty, type Schema } from "~/lib/api";

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

/**
 * `connectors.disconnect`: stops syncing for good and gives up the grant at
 * the provider. The connection leaves the cached list.
 */
export function useDisconnect(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (connectionId: string) => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/connections/{connection_id}", {
          params: { path: { workspace_id: workspaceId, connection_id: connectionId } },
        }),
      );
      return connectionId;
    },
    onSuccess: (connectionId) => {
      queryClient.setQueryData<Connection[]>(
        operationKey("connectors.list_connections", { workspace_id: workspaceId }),
        (connections) => connections?.filter((connection) => connection.id !== connectionId),
      );
    },
  });
}
