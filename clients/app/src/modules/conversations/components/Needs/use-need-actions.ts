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
import { useTeachOnce } from "../TeachOnce/TeachOnce";

const TOASTS: Record<Exclude<NeedVerdict, "reopen">, string> = {
  done: "needs.toast.done",
  not_needed: "needs.toast.notNeeded",
  confirm: "needs.toast.confirmed",
  already_paid: "needs.toast.paid",
  still_to_pay: "needs.toast.toPay",
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
  /**
   * A bill was marked paid and the server agreed: where the paid rule for
   * that sender is offered ("Mark their invoices paid from now on",
   * campbooks-cloud#169).
   */
  onPaid?: (conversation: ConversationSummary) => void;
}

/**
 * A member's verdicts on what a conversation needs (Done, Not needed or
 * "It's safe", Yes to a question), each pushed to the undo stack: Z or the
 * toast's Undo reopens it.
 */
export function useNeedActions(
  workspaceId: string,
  { onChanged, onUndo, onRedo, onFailed, onPaid }: NeedActionOptions = {},
) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const mutation = useConversationAction(workspaceId);
  const undo = useUndoToast();
  const teach = useTeachOnce();

  // Each request settles on its own promise: `mutate`'s callbacks only fire
  // for the latest call, so a failure under a quicker second action would be
  // lost (the row hidden, no toast). `entry` is the undo entry the verdict
  // pushed, taken back when it fails.
  const run = (conversation: ConversationSummary, verdict: NeedVerdict, entry?: number) => {
    void mutation
      .mutateAsync({ conversation, action: { need: verdict } })
      .then(
        () => {
          // Not needed on a need (not a warning) can teach: "these never need me".
          if (verdict === "not_needed" && conversation.need?.question !== "suspicious") {
            teach?.after(conversation, { kind: "not_needed" });
          }
          if (verdict === "already_paid") {
            onPaid?.(conversation);
          }
        },
        (error: unknown) => {
          // Nothing changed on the server: put it back as it was and say so
          // plainly. A failed reopen leaves it done (hidden again).
          if (entry !== undefined) {
            undo.retract(entry);
          }
          if (verdict === "reopen") {
            onRedo?.(conversation);
          } else {
            onFailed?.(conversation);
          }
          const need = conversation.need;
          const name = need === null || need === undefined ? "" : needSender(need, conversation);
          // A failure stays until Retry or dismiss. A need that changed
          // meanwhile is not retried: the member should look again.
          if (problemCode(error) === "need_changed") {
            undo.fail({ label: t("needs.toast.changed", { name }) });
          } else {
            undo.fail({
              label: t("needs.toast.failed", { name }),
              retry: () => {
                // Show at once what the retry is about to do.
                if (verdict === "reopen") {
                  onUndo?.(conversation);
                  run(conversation, "reopen");
                } else {
                  if (verdict !== "confirm" && verdict !== "still_to_pay") {
                    onRedo?.(conversation);
                  }
                  judge(conversation, verdict);
                }
              },
            });
          }
        },
      )
      .finally(() => {
        onChanged?.();
      });
  };

  const judge = (conversation: ConversationSummary, verdict: Exclude<NeedVerdict, "reopen">) => {
    const need = conversation.need;
    if (need === null || need === undefined) {
      return;
    }
    const name = needSender(need, conversation);
    const safe = verdict === "not_needed" && need.question === "suspicious";
    const entry = undo.push({
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
    run(conversation, verdict, entry);
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
    /** A bill is paid already: it leaves the list. */
    alreadyPaid: (conversation: ConversationSummary) => {
      judge(conversation, "already_paid");
    },
    /** A bill is still to pay: it stays, as Pay. */
    stillToPay: (conversation: ConversationSummary) => {
      judge(conversation, "still_to_pay");
    },
    /** Several bills paid at once, with one Undo for all of them. */
    allPaid: (conversations: readonly ConversationSummary[]) => {
      if (conversations.length === 0) {
        return;
      }
      const entry = undo.push({
        label: t("needs.toast.allPaid", { count: conversations.length }),
        undo: () => {
          for (const conversation of conversations) {
            onUndo?.(conversation);
            run(conversation, "reopen");
          }
        },
        redo: () => {
          for (const conversation of conversations) {
            onRedo?.(conversation);
            run(conversation, "already_paid");
          }
        },
      });
      for (const conversation of conversations) {
        run(conversation, "already_paid", entry);
      }
    },
    /** Take back a verdict (Done today's Undo). */
    reopen: (conversation: ConversationSummary) => {
      run(conversation, "reopen");
    },
    /** The verdict that failed, if one did. */
    failed: mutation.isError,
  };
}
