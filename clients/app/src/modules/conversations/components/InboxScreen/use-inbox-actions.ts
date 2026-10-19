import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import {
  useConversationAction,
  type ConversationAction,
  type ConversationSummary,
  type Stream,
} from "../../api/queries";
import type { TaughtAction } from "../../api/rules";
import { streamName, undoMoves, type Move, type Translate } from "../../model";
import { useTeachOnce } from "../TeachOnce/TeachOnce";

/** Each move's toast. */
const TOASTS: Record<Move, string> = {
  archive: "toast.done",
  unarchive: "toast.toInbox",
  trash: "toast.deleted",
  untrash: "toast.restored",
  spam: "toast.spam",
  not_spam: "toast.notSpam",
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
  /** Run `moves` one after the other, each on the summary the last one returned. */
  const runInOrder = (conversation: ConversationSummary, moves: readonly Move[]) => {
    const [first, ...rest] = moves;
    if (first === undefined) {
      return;
    }
    mutation.mutate(
      { conversation, action: first },
      {
        onSuccess: (summary) => {
          teach?.exception(summary.rule_exception);
          runInOrder(summary, rest);
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
      // One toast and one Undo entry, which puts it back exactly where it
      // was, in as many steps as that takes.
      const back = undoMoves(conversation, move);
      run(conversation, move, move === "archive" ? { kind: "archive" } : undefined);
      undo.push({
        label: t(TOASTS[move], { subject: subjectOf(conversation) }),
        undo: () => {
          runInOrder(conversation, back);
        },
        redo: () => {
          run(conversation, move);
        },
      });
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
