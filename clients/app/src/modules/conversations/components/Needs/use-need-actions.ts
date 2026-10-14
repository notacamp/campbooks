import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useUndoToast } from "~/lib/ui";

import {
  useConversationAction,
  type ConversationSummary,
  type NeedVerdict,
} from "../../api/queries";
import type { Translate } from "../../model";
import { needSender } from "../../needs";

const TOASTS: Record<Exclude<NeedVerdict, "reopen">, string> = {
  done: "needs.toast.done",
  not_needed: "needs.toast.notNeeded",
  confirm: "needs.toast.confirmed",
};

export interface NeedActionOptions {
  /** After each request settles (Home refetches its list). */
  onChanged?: () => void;
  /** When a verdict is undone (Home shows the item again at once). */
  onUndo?: (conversation: ConversationSummary) => void;
  /** When an undone verdict is redone. */
  onRedo?: (conversation: ConversationSummary) => void;
  /** When the server refused or failed: Home shows the item again. */
  onFailed?: (conversation: ConversationSummary) => void;
}

/**
 * A member's verdicts on what a conversation needs (Done, Not needed or
 * "It's safe", Yes to a question), each pushed to the undo stack: Z or the
 * toast's Undo reopens it.
 */
export function useNeedActions(
  workspaceId: string,
  { onChanged, onUndo, onRedo, onFailed }: NeedActionOptions = {},
) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const mutation = useConversationAction(workspaceId);
  const undo = useUndoToast();

  const run = (conversation: ConversationSummary, verdict: NeedVerdict) => {
    mutation.mutate(
      { conversation, action: { need: verdict } },
      {
        onError: (error) => {
          // Nothing changed on the server: put it back and say so plainly.
          onFailed?.(conversation);
          const need = conversation.need;
          const name = need === null || need === undefined ? "" : needSender(need, conversation);
          undo.notify(
            t(
              problemCode(error) === "need_changed" ? "needs.toast.changed" : "needs.toast.failed",
              {
                name,
              },
            ),
          );
        },
        onSettled: () => {
          onChanged?.();
        },
      },
    );
  };

  const judge = (conversation: ConversationSummary, verdict: Exclude<NeedVerdict, "reopen">) => {
    const need = conversation.need;
    if (need === null || need === undefined) {
      return;
    }
    const name = needSender(need, conversation);
    const safe = verdict === "not_needed" && need.question === "suspicious";
    run(conversation, verdict);
    undo.push({
      label: t(safe ? "needs.toast.safe" : TOASTS[verdict], { name }),
      undo: () => {
        onUndo?.(conversation);
        run(conversation, "reopen");
      },
      redo: () => {
        onRedo?.(conversation);
        run(conversation, verdict);
      },
    });
  };

  return {
    done: (conversation: ConversationSummary) => {
      judge(conversation, "done");
    },
    notNeeded: (conversation: ConversationSummary) => {
      judge(conversation, "not_needed");
    },
    confirm: (conversation: ConversationSummary) => {
      judge(conversation, "confirm");
    },
    /** Take back a verdict (Done today's Undo). */
    reopen: (conversation: ConversationSummary) => {
      run(conversation, "reopen");
    },
    /** The verdict that failed, if one did. */
    failed: mutation.isError,
  };
}
