import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import { useNotionCalls, useRefreshNotion } from "../api/notion";
import { useRefreshPages } from "../api/queries";
import type { ImportFromNotionBody, Imported, NotionKeep } from "../types";

/**
 * A member's dealings with Notion on pages: settle a conflict (with an Undo
 * in the toast), stop syncing, import. A failure is said in a sentence;
 * nothing is ever half done on this side.
 */
export function useNotionActions(workspaceId: string) {
  const { t } = useTranslation("pages");
  const calls = useNotionCalls(workspaceId);
  const refresh = useRefreshNotion(workspaceId);
  const refreshPages = useRefreshPages(workspaceId);
  const undo = useUndoToast();

  const resolve = async (page: string, block: string, keep: NotionKeep): Promise<boolean> => {
    try {
      const settled = await calls.resolve(page, block, keep);
      refresh(page);
      undo.push({
        label: t("notion.conflict.done", { what: t(`notion.conflict.resolved.${keep}`) }),
        undo: () => {
          void calls.undo(page, settled.resolution).then(
            () => {
              refresh(page);
            },
            () => {
              refresh(page);
              undo.fail({ label: t("notion.conflict.undoFailed") });
            },
          );
        },
      });
      return true;
    } catch {
      refresh(page);
      undo.fail({
        label: t("notion.conflict.failed"),
        retry: () => {
          void resolve(page, block, keep);
        },
      });
      return false;
    }
  };

  const stop = async (page: string): Promise<boolean> => {
    try {
      await calls.stop(page);
      refresh(page);
      undo.notify(t("notion.sync.stopped"));
      return true;
    } catch {
      refresh(page);
      undo.fail({ label: t("notion.sync.stopFailed") });
      return false;
    }
  };

  const importPages = async (body: ImportFromNotionBody): Promise<Imported | undefined> => {
    try {
      const imported = await calls.import(body);
      refreshPages();
      refresh();
      undo.notify(t("notion.import.done", { count: imported.pages.length }));
      return imported;
    } catch {
      return undefined;
    }
  };

  return { resolve, stop, importPages };
}
