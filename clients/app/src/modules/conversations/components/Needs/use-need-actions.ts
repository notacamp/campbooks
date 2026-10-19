import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useUndoToast } from "~/lib/ui";

import {
  useConversationAction,
  type ConversationSummary,
  type NeedVerdict,
} from "../../api/queries";
import type { Translate } from "../../model";
import { isStillNeeded, needSender } from "../../needs";
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
  /**
   * A warning was reported as spam and the server agreed: where the rule
   * for mail like it is offered ("Never see emails like this again?",
   * campbooks-cloud#169).
   */
  onReported?: (conversation: ConversationSummary) => void;
}

/**
 * A member's verdicts on what a conversation needs (Done, Not needed or
 * "It's safe", Yes to a question), each pushed to the undo stack: Z or the
 * toast's Undo reopens it.
 */
export function useNeedActions(
  workspaceId: string,
  { onChanged, onUndo, onRedo, onFailed, onPaid, onReported }: NeedActionOptions = {},
) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const mutation = useConversationAction(workspaceId);
  const undo = useUndoToast();
  const teach = useTeachOnce();

  // Each request settles on its own promise: `mutate`'s callbacks only fire
  // for the latest call, so a failure under a quicker second action would be
  // lost (the row hidden, no toast). `entry` is the undo entry the verdict
  // pushed, taken back when it fails. `undone` says when that entry was
  // undone before the server answered: then nothing is offered after it.
  const run = (
    conversation: ConversationSummary,
    verdict: NeedVerdict,
    entry?: number,
    undone?: { current: boolean },
  ) => {
    void mutation
      .mutateAsync({ conversation, action: { need: verdict } })
      .then(
        () => {
          // Not needed on a need (not a warning) can teach: "these never need me".
          // Not on "Still needed?": the mail did need something, time passed.
          const need = conversation.need;
          if (
            verdict === "not_needed" &&
            need !== null &&
            need !== undefined &&
            need.question !== "suspicious" &&
            !isStillNeeded(need)
          ) {
            teach?.after(conversation, { kind: "not_needed" });
          }
          if (verdict === "already_paid" && undone?.current !== true) {
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
    // "Still needed?" answered No: nothing is learned from it, so say only that.
    const stale = verdict === "not_needed" && isStillNeeded(need);
    const label = safe
      ? "needs.toast.safe"
      : stale
        ? "needs.toast.noLongerNeeded"
        : TOASTS[verdict];
    const undone = { current: false };
    const entry = undo.push({
      label: t(label, { name }),
      undo: () => {
        undone.current = true;
        onUndo?.(conversation);
        run(conversation, "reopen");
      },
      redo: () => {
        undone.current = false;
        // Yes to a question and Still to pay keep the item on the list.
        if (verdict !== "confirm" && verdict !== "still_to_pay") {
          onRedo?.(conversation);
        }
        run(conversation, verdict, undefined, undone);
      },
    });
    run(conversation, verdict, entry, undone);
  };

  /** One Undo for several bills marked paid at once: those the server took. */
  const pushAllPaid = (conversations: readonly ConversationSummary[]) =>
    undo.push({
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

  // Several bills at once: each request settles on its own; when some fail,
  // the Undo covers only the others and one failure says so, with one Retry
  // for those. No paid rule is offered after a bulk action.
  const allPaid = (conversations: readonly ConversationSummary[]) => {
    if (conversations.length === 0) {
      return;
    }
    const entry = pushAllPaid(conversations);
    void Promise.allSettled(
      conversations.map((conversation) =>
        mutation.mutateAsync({ conversation, action: { need: "already_paid" } }),
      ),
    ).then((results) => {
      const failed: ConversationSummary[] = [];
      // A bill that changed while the member looked is not retried, as for
      // one bill: they should look at it again.
      const retryable: ConversationSummary[] = [];
      for (const [index, result] of results.entries()) {
        const conversation = conversations[index];
        if (result.status === "rejected" && conversation !== undefined) {
          failed.push(conversation);
          if (problemCode(result.reason) !== "need_changed") {
            retryable.push(conversation);
          }
        }
      }
      if (failed.length > 0) {
        undo.retract(entry);
        const done = conversations.filter((conversation) => !failed.includes(conversation));
        if (done.length > 0) {
          pushAllPaid(done);
        }
        for (const conversation of failed) {
          onFailed?.(conversation);
        }
        undo.fail({
          label: t("needs.toast.allPaidFailed", { count: failed.length }),
          retry:
            retryable.length === 0
              ? undefined
              : () => {
                  for (const conversation of retryable) {
                    onRedo?.(conversation);
                  }
                  allPaid(retryable);
                },
        });
      }
      onChanged?.();
    });
  };

  /**
   * Report and delete: the conversation goes to spam (at the provider too),
   * with Undo putting it back where it was.
   */
  const report = (conversation: ConversationSummary) => {
    const need = conversation.need;
    const name = need === null || need === undefined ? "" : needSender(need, conversation);
    // Undone before the server answered: no offer for mail taken back out.
    let undone = false;
    // Not spam waits for the report to settle, so it is never overtaken,
    // and is sent only when the report went through.
    let reported: Promise<boolean> = Promise.resolve(false);
    const entry = undo.push({
      label: t("needs.toast.reported", { name }),
      undo: () => {
        undone = true;
        onUndo?.(conversation);
        void reported
          .then((ok) =>
            ok ? mutation.mutateAsync({ conversation, action: "not_spam" }) : undefined,
          )
          .catch(() => {
            undo.notify(t("needs.toast.failed", { name }));
          })
          .finally(() => {
            onChanged?.();
          });
      },
    });
    reported = mutation
      .mutateAsync({ conversation, action: "spam" })
      .then(
        () => {
          if (!undone) {
            onReported?.(conversation);
          }
          return true;
        },
        () => {
          undo.retract(entry);
          onFailed?.(conversation);
          undo.fail({
            label: t("needs.toast.failed", { name }),
            retry: () => {
              onRedo?.(conversation);
              report(conversation);
            },
          });
          return false;
        },
      )
      .finally(() => {
        onChanged?.();
      });
  };

  return {
    /** Report and delete a warning: into spam, with Undo. */
    report,
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
    allPaid,
    /** Take back a verdict (Done today's Undo). */
    reopen: (conversation: ConversationSummary) => {
      run(conversation, "reopen");
    },
    /** The verdict that failed, if one did. */
    failed: mutation.isError,
  };
}
