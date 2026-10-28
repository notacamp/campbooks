import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import { createDraft, deleteDraft, draftKeys, type Draft } from "../api/drafts";
import type { Translate } from "../model";

/**
 * Delete a draft the member can still edit, wherever it is shown: it goes
 * from every cached list at once and a "Draft deleted" toast offers Undo,
 * which writes the same draft again (same kind, message, recipients, subject,
 * text and quoted history). Forwarded attachments come back from the message
 * as a new forward would have them, so any the member had removed return too.
 * Uploaded files do not: their blobs are released when the draft goes, so Undo
 * says plainly to attach them again.
 * Resolves once it is gone; rejects if the server refused (nothing changed).
 */
export function useDeleteDraft(workspaceId: string) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const queryClient = useQueryClient();
  const undo = useUndoToast();

  const refresh = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
  }, [queryClient, workspaceId]);

  const restore = useCallback(
    async (draft: Draft) => {
      try {
        const back = await createDraft(workspaceId, {
          kind: draft.kind,
          inbox_id: draft.inbox_id,
          message_id: draft.message_id ?? null,
          to: draft.to,
          cc: draft.cc,
          bcc: draft.bcc,
          subject: draft.subject,
          body: draft.body,
          quoted: draft.quoted,
        });
        const uploaded = draft.attachments.filter((a) => a.source === "uploaded").length;
        if (uploaded > 0) {
          undo.notify(t("compose.draftRestoredWithoutFiles", { count: uploaded }));
        }
        if (back.conversation_id !== null && back.conversation_id !== undefined) {
          queryClient.setQueryData<Draft[]>(
            draftKeys.forConversation(workspaceId, back.conversation_id),
            (list) => (list === undefined ? list : [back, ...list.filter((d) => d.id !== back.id)]),
          );
        }
      } catch {
        undo.notify(t("compose.draftNotRestored"));
      } finally {
        await refresh();
      }
    },
    [queryClient, refresh, t, undo, workspaceId],
  );

  return useCallback(
    async (draft: Draft): Promise<void> => {
      await deleteDraft(workspaceId, draft.id);
      queryClient.setQueriesData<Draft[]>({ queryKey: draftKeys.all(workspaceId) }, (list) =>
        list?.filter((d) => d.id !== draft.id),
      );
      undo.push({
        label: t("compose.draftDeleted"),
        undo: () => {
          void restore(draft);
        },
      });
      await refresh();
    },
    [queryClient, refresh, restore, t, undo, workspaceId],
  );
}
