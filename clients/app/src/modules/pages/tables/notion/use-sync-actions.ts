import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import { tableKeys } from "../api";

import { syncKey, useTableSyncCalls } from "./api";
import type { TableConflictKeep } from "./model";

/**
 * A member's dealings with a synced table: settle a conflict (with an Undo
 * in the toast) and stop syncing. A failure is said in a sentence and nothing
 * is half done on this side.
 */
export function useTableSyncActions(workspaceId: string, tableId: string) {
  const { t } = useTranslation("pages");
  const client = useQueryClient();
  const calls = useTableSyncCalls(workspaceId, tableId);
  const undo = useUndoToast();

  // A choice can move a cell, a column or the whole schema: read it all again.
  const refresh = () => {
    void client.invalidateQueries({ queryKey: syncKey(workspaceId, tableId) });
    void client.invalidateQueries({ queryKey: tableKeys.table(workspaceId, tableId) });
    void client.invalidateQueries({ queryKey: tableKeys.rows(workspaceId, tableId) });
  };

  const resolve = async (conflict: string, keep: TableConflictKeep): Promise<boolean> => {
    try {
      await calls.resolve(conflict, keep);
      refresh();
      undo.push({
        label: t("notion.conflict.done", {
          what: t(`notion.conflict.resolved.${keep === "ours" ? "mine" : "notion"}`),
        }),
        undo: () => {
          void calls.undo(conflict).then(refresh, () => {
            refresh();
            undo.fail({ label: t("notion.conflict.undoFailed") });
          });
        },
      });
      return true;
    } catch {
      refresh();
      undo.fail({
        label: t("notion.conflict.failed"),
        retry: () => {
          void resolve(conflict, keep);
        },
      });
      return false;
    }
  };

  const stop = async (): Promise<boolean> => {
    try {
      await calls.stop();
      refresh();
      undo.notify(t("notion.sync.stopped"));
      return true;
    } catch {
      refresh();
      undo.fail({ label: t("notion.sync.stopFailed") });
      return false;
    }
  };

  return { resolve, stop };
}

export type TableSyncActions = ReturnType<typeof useTableSyncActions>;
