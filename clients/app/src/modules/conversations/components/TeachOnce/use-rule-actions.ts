import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import {
  useRuleMutations,
  type RuleAction,
  type RuleCondition,
  type RuleView,
} from "../../api/rules";
import type { Translate } from "../../model";
import { ruleSentence } from "../../rules-text";

/** Which toast a removal shows. */
type RemovalLabel = "stopped";

/** What a removal needs to know of a rule: a row's mark is enough. */
interface RuleRef {
  id: string;
  conditions: readonly RuleCondition[];
  actions: readonly RuleAction[];
}

/**
 * Removing and keeping rules with the feedback every action gets: a toast
 * once the server said yes (with Undo), a plain error toast when it said no.
 * Undo after a removal brings the same rule back as it was
 * (`conversations.restore_rule`, applied again when its mail was put back).
 * A stale Undo never removes a rule twice.
 */
export function useRuleActions(workspaceId: string) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const undo = useUndoToast();
  const {
    create,
    remove,
    confirm,
    decline,
    restore: bringBack,
    wasRemoved,
  } = useRuleMutations(workspaceId);
  const failed = () => {
    undo.notify(t("rules.toast.failed"));
  };
  const sentence = (rule: RuleRef) => ruleSentence(t, i18n.language, rule);

  /** Bring removed `ruleId` back as it was (applied again when `apply`). */
  const restoreRule = (ruleId: string, apply: boolean) => {
    bringBack.mutate({ ruleId, applyToExisting: apply }, { onError: failed });
  };

  /** Remove `ruleId` (putting its mail back when `restore`), unless this screen already did. */
  const removeOnce = (ruleId: string, restore: boolean, onSuccess?: () => void) => {
    if (wasRemoved(ruleId)) {
      return;
    }
    remove.mutate(
      { ruleId, restore },
      {
        onSuccess: () => {
          onSuccess?.();
        },
        onError: failed,
      },
    );
  };

  /** Remove `rule`; `onDone` once it is gone. */
  const removeRule = (
    rule: RuleRef,
    {
      restore,
      label = "stopped",
      onDone,
      failure,
    }: {
      restore: boolean;
      label?: RemovalLabel | undefined;
      onDone?: (() => void) | undefined;
      /** What to say when it fails (default: the rule did not change). */
      failure?: string | undefined;
    },
  ) => {
    remove.mutate(
      { ruleId: rule.id, restore },
      {
        onSuccess: () => {
          onDone?.();
          undo.push({
            label: t(`rules.toast.${label}`, { rule: sentence(rule) }),
            undo: () => {
              restoreRule(rule.id, restore);
            },
          });
        },
        onError: () => {
          if (failure === undefined) {
            failed();
          } else {
            undo.notify(failure);
          }
        },
      },
    );
  };

  /** Turn a proposed `rule` down; Undo brings the proposal back. */
  const declineRule = (rule: RuleView) => {
    decline.mutate(
      { ruleId: rule.id },
      {
        onSuccess: () => {
          undo.push({
            label: t("rules.toast.declined", { rule: sentence(rule) }),
            undo: () => {
              restoreRule(rule.id, false);
            },
          });
        },
        onError: failed,
      },
    );
  };

  /**
   * Say yes to a proposed `rule`; Undo removes it and puts back what it
   * changed. Kept but paused (its author is away for now), the toast says so.
   */
  const keepRule = (rule: RuleView) => {
    confirm.mutate(
      { ruleId: rule.id },
      {
        onSuccess: (kept) => {
          undo.push({
            label: t(kept.state === "paused" ? "rules.toast.keptPaused" : "rules.toast.kept", {
              rule: sentence(rule),
            }),
            undo: () => {
              removeOnce(rule.id, true);
            },
          });
        },
        onError: failed,
      },
    );
  };

  return {
    removeRule,
    removeOnce,
    restoreRule,
    keepRule,
    declineRule,
    declining: decline.isPending,
    removing: remove.isPending,
    keeping: confirm.isPending,
    create,
  };
}
