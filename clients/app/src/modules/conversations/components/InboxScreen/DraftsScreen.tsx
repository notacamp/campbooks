import { useNavigate } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import {
  EmptyState,
  Heading,
  ListPane,
  PaneHeader,
  ReaderPane,
  Spacer,
  useUndoToast,
} from "~/lib/ui";

import { useDrafts, type Draft } from "../../api/drafts";
import { useDeleteDraft } from "../../compose/use-delete-draft";
import type { Translate } from "../../model";
import { DraftsList } from "../DraftsList/DraftsList";
import { StreamChips } from "../StreamNav/StreamNav";

export interface DraftsScreenProps {
  workspaceId: string;
  /** A list-header action from the send slice ("New message", C). */
  composeAction?: ReactNode | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * Drafts (`/inbox/drafts`): your saved drafts, a place beside the views whose
 * list is `conversations.list_drafts`. Opening one opens the composer where
 * it was left: a new message on its own screen, an answer under its thread.
 */
export function DraftsScreen({ workspaceId, composeAction, now }: DraftsScreenProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const drafts = useDrafts(workspaceId);
  const deleteDraft = useDeleteDraft(workspaceId);
  const toast = useUndoToast();

  const open = (draft: Draft) => {
    if (draft.conversation_id === null || draft.conversation_id === undefined) {
      void navigate({
        to: "/w/$workspaceId/compose",
        params: { workspaceId },
        search: { draft: draft.id },
      });
      return;
    }
    // The thread reopens the draft it holds; a reply names its message, so
    // that one is the one that opens.
    void navigate({
      to: "/w/$workspaceId/inbox/$stream/$conversationId",
      params: { workspaceId, stream: "primary", conversationId: draft.conversation_id },
      search:
        draft.kind === "reply" && draft.message_id !== null && draft.message_id !== undefined
          ? { reply: true, to: draft.message_id }
          : {},
    });
  };

  return (
    <>
      <ListPane label={t("drafts.label")}>
        <PaneHeader>
          <Heading level={1} size="pane">
            {t("views.drafts")}
          </Heading>
          <Spacer />
          {composeAction}
        </PaneHeader>
        <div className="px-3 pt-2 md:hidden">
          <StreamChips workspaceId={workspaceId} current="drafts" />
        </div>
        <DraftsList
          drafts={drafts.data}
          failed={drafts.isError}
          onRetry={() => {
            void drafts.refetch();
          }}
          onOpen={open}
          onDelete={(draft) => {
            void deleteDraft(draft).catch(() => {
              toast.notify(t("compose.errors.unknown"));
            });
          }}
          now={now}
        />
      </ListPane>
      <ReaderPane label={t("reader.label")} narrow="hidden">
        <div className="flex flex-col items-center gap-2 p-6">
          <EmptyState title={t("drafts.readerTitle")} visor="idle" visorSize="lg">
            {t("drafts.readerBody")}
          </EmptyState>
        </div>
      </ReaderPane>
    </>
  );
}
