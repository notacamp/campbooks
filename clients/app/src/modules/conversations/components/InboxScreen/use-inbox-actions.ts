import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import {
  useConversationAction,
  type ConversationAction,
  type ConversationSummary,
  type Stream,
} from "../../api/queries";
import type { TaughtAction } from "../../api/rules";
import { streamName, type Translate } from "../../model";
import { useTeachOnce } from "../TeachOnce/TeachOnce";

/**
 * The inbox's actions on one conversation, each pushed to the undo stack
 * with its toast (Z or the toast's Undo takes it back): Done (archive),
 * star, read/unread, move to a stream.
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
    done: (conversation: ConversationSummary) => {
      undoable(
        conversation,
        "archive",
        "unarchive",
        t("toast.done", { subject: subjectOf(conversation) }),
        { kind: "archive" },
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
