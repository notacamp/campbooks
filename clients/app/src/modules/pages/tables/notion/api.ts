import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { api, errorStatus, operationKey, unwrap } from "~/lib/api";

import type { TableConflictKeep, TableSync } from "./model";

// Keyed by operationId, so the change feed refetches it when the table's
// page changes (a sync moved, a conflict was raised).
const SYNC = "pages.get_table_sync";

export const syncKey = (workspaceId: string, tableId: string) =>
  operationKey(SYNC, { workspace_id: workspaceId, table_id: tableId });

const NOT_SYNCED: TableSync = { link: null, conflicts: [] };

/**
 * `pages.get_table_sync`: whether a table is kept in step with a Notion
 * database, and the changes waiting for a choice. A server without Notion
 * databases answers 404: that is not synced, not a failure.
 */
export function useTableSync(workspaceId: string, tableId: string) {
  return useQuery({
    queryKey: syncKey(workspaceId, tableId),
    retry: false,
    queryFn: async ({ signal }): Promise<TableSync> => {
      try {
        return unwrap(
          await api.GET("/api/v1/w/{workspace_id}/tables/{table_id}/notion", {
            params: { path: { workspace_id: workspaceId, table_id: tableId } },
            signal,
          }),
        );
      } catch (error) {
        if (errorStatus(error) === 404) {
          return NOT_SYNCED;
        }
        throw error;
      }
    },
  });
}

/** The calls a member makes on a synced table. */
export function useTableSyncCalls(workspaceId: string, tableId: string) {
  return useMemo(() => {
    const path = { workspace_id: workspaceId, table_id: tableId };
    return {
      resolve: async (conflict: string, keep: TableConflictKeep) =>
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/notion/conflicts/{conflict}", {
            params: { path: { ...path, conflict } },
            body: { keep },
          }),
        ),
      undo: async (conflict: string) =>
        unwrap(
          await api.POST(
            "/api/v1/w/{workspace_id}/tables/{table_id}/notion/conflicts/{conflict}/undo",
            { params: { path: { ...path, conflict } } },
          ),
        ),
      stop: async () =>
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/notion/stop", {
            params: { path },
          }),
        ),
    };
  }, [workspaceId, tableId]);
}
