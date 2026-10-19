import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import {
  useConversationAction,
  type ConversationAction,
  type ConversationSummary,
  type Stream,
} from "../../api/queries";
import type { TaughtAction } from "../../api/rules";
import { streamName, type Move, type Translate } from "../../model";
import { useTeachOnce } from "../TeachOnce/TeachOnce";

/** Each move's inverse (its Undo) and its toast. */
const MOVES: Record<Move, { inverse: Move; toast: string }> = {
  archive: { inverse: "unarchive", toast: "toast.done" },
  unarchive: { inverse: "archive", toast: "toast.toInbox" },
  trash: { inverse: "untrash", toast: "toast.deleted" },
  untrash: { inverse: "trash", toast: "toast.restored" },
  spam: { inverse: "not_spam", toast: "toast.spam" },
  not_spam: { inverse: "spam", toast: "toast.notSpam" },
};

/**
 * The inbox's actions on one conversation, each pushed to the undo stack
 * with its toast (Z or the toast's Undo takes it back): Done (archive) and
 * Move to Inbox, Delete and Restore, Report spam and Not spam, star,
 * read/unread, move to a stream.
 */
export function useInboxActions(workspaceId: string) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const mutation = useConversationAction(workspaceId);
  const undo = useUndoToast();
  const teach = useTeachOnce();

  /**
   * Run `action`; once the server has it, offer to teach `taught` (in the
   * inbox), and hand any rule it changed back to teach-once.
   */
  const run = (
    conversation: ConversationSummary,
    action: ConversationAction,
    taught?: TaughtAction,
  ) => {
    mutation.mutate(
      { conversation, action },
      {
        onSuccess: (summary) => {
          teach?.exception(summary.rule_exception);
          if (taught !== undefined) {
            teach?.after(conversation, taught);
          }
        },
      },
    );
  };
  const subjectOf = (conversation: ConversationSummary) =>
    conversation.subject === "" ? t("reader.noSubject") : conversation.subject;

  /** Push `action` with its inverse. */
  const undoable = (
    conversation: ConversationSummary,
    action: ConversationAction,
    inverse: ConversationAction,
    label: string,
    taught?: TaughtAction,
  ) => {
    run(conversation, action, taught);
    undo.push({
      label,
      undo: () => {
        run(conversation, inverse);
      },
      redo: () => {
        run(conversation, action);
      },
    });
  };

  return {
    /** The subject of the conversation the last action failed on, if it did. */
    failedOn: mutation.isError ? subjectOf(mutation.variables.conversation) : undefined,
    dismissFailure: () => {
      mutation.reset();
    },
    /** Move it to another place; Done (archive) offers to teach archiving. */
    moveTo: (conversation: ConversationSummary, move: Move) => {
      const { inverse, toast } = MOVES[move];
      undoable(
        conversation,
        move,
        inverse,
        t(toast, { subject: subjectOf(conversation) }),
        move === "archive" ? { kind: "archive" } : undefined,
      );
    },
    toggleStar: (conversation: ConversationSummary) => {
      const starring = !conversation.starred;
      undoable(
        conversation,
        starring ? "star" : "unstar",
        starring ? "unstar" : "star",
        t(starring ? "toast.starred" : "toast.unstarred", { subject: subjectOf(conversation) }),
      );
    },
    toggleRead: (conversation: ConversationSummary) => {
      const unread = conversation.unread_count > 0;
      undoable(
        conversation,
        unread ? "read" : "unread",
        unread ? "unread" : "read",
        t(unread ? "toast.read" : "toast.unread", { subject: subjectOf(conversation) }),
        unread ? { kind: "mark_read" } : undefined,
      );
    },
    /** Opening an unread conversation reads it: no toast. */
    markRead: (conversation: ConversationSummary) => {
      run(conversation, "read");
    },
    move: (conversation: ConversationSummary, stream: Stream) => {
      if (stream === conversation.stream) {
        return;
      }
      undoable(
        conversation,
        { stream },
        { stream: conversation.stream },
        t("toast.moved", { stream: streamName(t, stream), subject: subjectOf(conversation) }),
        { kind: "move_to_stream", stream },
      );
    },
  };
}

export type InboxActions = ReturnType<typeof useInboxActions>;
