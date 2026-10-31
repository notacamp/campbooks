import { useQueryClient } from "@tanstack/react-query";
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

import { cancelSend, draftKeys, useDrafts, type Draft } from "../../api/drafts";
import { useDeleteDraft } from "../../compose/use-delete-draft";
import type { Translate } from "../../model";
import { DraftsList } from "../DraftsList/DraftsList";
import { ConversationReader } from "../Reader/ConversationReader";
import type { RenderReplySlot } from "../Reader/ReplySlot";
import { StreamChips } from "../StreamNav/StreamNav";

import { useInboxActions } from "./use-inbox-actions";

export interface DraftsScreenProps {
  workspaceId: string;
  /** The conversation open beside the list (a draft's thread), if any. */
  conversationId?: string | undefined;
  /** The draft the composer opens on, whatever its kind. */
  draftId?: string | undefined;
  /** The send slice's composer, for the thread's reply slot. */
  renderReplySlot?: RenderReplySlot | undefined;
  /** A list-header action from the send slice ("New message", C). */
  composeAction?: ReactNode | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * Drafts (`/inbox/drafts`): your saved drafts, a place beside the views whose
 * list is `conversations.list_drafts`. Opening one opens the composer where
 * it was left: a new message on its own screen, an answer in its thread beside the list.
 */
export function DraftsScreen({
  workspaceId,
  conversationId,
  draftId,
  renderReplySlot,
  composeAction,
  now,
}: DraftsScreenProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const drafts = useDrafts(workspaceId);
  const deleteDraft = useDeleteDraft(workspaceId);
  const toast = useUndoToast();
  const queryClient = useQueryClient();
  const actions = useInboxActions(workspaceId);
  const toList = () => {
    void navigate({
      to: "/w/$workspaceId/inbox/$stream",
      params: { workspaceId, stream: "drafts" },
    });
  };

  /** Take a scheduled send back (campbooks-cloud#392): the draft, or nothing when too late. */
  const takeBack = async (draft: Draft): Promise<Draft | undefined> => {
    try {
      return await cancelSend(workspaceId, draft.id);
    } catch {
      toast.notify(t("compose.tooLate"));
      return undefined;
    } finally {
      void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
    }
  };

  const open = (draft: Draft) => {
    if (draft.conversation_id === null || draft.conversation_id === undefined) {
      void navigate({
        to: "/w/$workspaceId/compose",
        params: { workspaceId },
        search: { draft: draft.id },
      });
      return;
    }
    // Its thread opens beside the list, on exactly this draft.
    void navigate({
      to: "/w/$workspaceId/inbox/$stream/$conversationId",
      params: { workspaceId, stream: "drafts", conversationId: draft.conversation_id },
      search: { draft: draft.id },
    });
  };

  return (
    <>
      <ListPane
        label={t("drafts.label")}
        narrow={conversationId === undefined ? "shown" : "hidden"}
      >
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
          onCancelSend={(draft) => {
            void takeBack(draft).then((back) => {
              if (back !== undefined) {
                toast.notify(t("drafts.sendCancelled"));
              }
            });
          }}
          onCancelAndEdit={(draft) => {
            void takeBack(draft).then((back) => {
              if (back !== undefined) {
                toast.notify(t("drafts.sendCancelled"));
                open(back);
              }
            });
          }}
          now={now}
        />
      </ListPane>
      <ReaderPane
        label={t("reader.label")}
        narrow={conversationId === undefined ? "hidden" : "shown"}
      >
        {conversationId === undefined ? (
          <div className="flex flex-col items-center gap-2 p-6">
            <EmptyState title={t("drafts.readerTitle")} visor="idle" visorSize="lg">
              {t("drafts.readerBody")}
            </EmptyState>
          </div>
        ) : (
          <ConversationReader
            key={`${conversationId}:${draftId ?? ""}`}
            workspaceId={workspaceId}
            conversationId={conversationId}
            stream="primary"
            backLabel={t("reader.back", { stream: t("views.drafts") })}
            actions={actions}
            onBack={toList}
            onPrevious={toList}
            onNext={toList}
            onMove={(conversation, move) => {
              actions.moveTo(conversation, move);
              toList();
            }}
            renderReplySlot={renderReplySlot}
            draftId={draftId}
            now={now}
          />
        )}
      </ReaderPane>
    </>
  );
}
