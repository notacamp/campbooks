import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Checkbox,
  Dialog,
  RulePreview,
  Text,
  useUndoToast,
  type RulePreviewSample,
} from "~/lib/ui";

import {
  useRuleMutations,
  useRulePreview,
  type RuleAction,
  type RuleCondition,
  type RuleView,
} from "../../api/rules";
import type { Translate } from "../../model";
import { ruleSentence } from "../../rules-text";

/** Examples the confirmation shows. */
const EXAMPLES = 3;

/** A rule an offer proposes, before the member confirms it. */
export interface SmartRuleDraft {
  /** The inbox it would be saved in. */
  inboxId: string;
  conditions: RuleCondition[];
  actions: RuleAction[];
  /** The dialog's title: "A rule for mail from notifications@github.com". */
  title: string;
  /** Where it came from (an action by default; `suggestion` for Scout noticed). */
  origin?: RuleView["origin"] | undefined;
  /** The conversation it was taught from. */
  fromConversation?: string | undefined;
  /** The "Scout noticed" suggestion it answers. */
  noticeId?: string | undefined;
  /** The inbox's address when other members use it: the rule acts for them too. */
  sharedInbox?: string | undefined;
}

export interface SmartRuleConfirmProps {
  workspaceId: string;
  /** The rule to confirm; `null` while closed. */
  draft: SmartRuleDraft | null;
  /** Cancel (nothing saved), or once the result was shown. */
  onClose: () => void;
  /** Saved: the rule, its run started when it applies to the mail already here. */
  onSaved?: ((rule: RuleView) => void) | undefined;
}

/**
 * The one confirmation every offered rule goes through before anything is
 * saved (campbooks-cloud#169): the rule as a sentence labelled "Smart
 * rule", that it runs on its own with no AI and lives in Scout's memory,
 * what it matches (a count and a few subjects, as plain text), whether to
 * apply it to the mail already here, then Save smart rule (⌘↵) or Cancel
 * (Esc). Saving shows its result for about a second; the toast's Undo
 * removes the rule and puts back what it changed.
 */
export function SmartRuleConfirm({ workspaceId, draft, onClose, onSaved }: SmartRuleConfirmProps) {
  if (draft === null) {
    return null;
  }
  return (
    <Confirm
      // A new draft starts over (its own preview, the box checked).
      key={`${draft.inboxId}:${JSON.stringify(draft.conditions)}:${JSON.stringify(draft.actions)}`}
      workspaceId={workspaceId}
      draft={draft}
      onClose={onClose}
      onSaved={onSaved}
    />
  );
}

function Confirm({
  workspaceId,
  draft,
  onClose,
  onSaved,
}: SmartRuleConfirmProps & { draft: SmartRuleDraft }) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const locale = i18n.language;
  const undo = useUndoToast();
  const { create, remove, restore, wasRemoved } = useRuleMutations(workspaceId);
  const preview = useRulePreview(workspaceId, draft.inboxId, {
    conditions: draft.conditions,
    actions: draft.actions,
  });
  const [existing, setExisting] = useState(true);
  const [failed, setFailed] = useState(false);
  const [saved, setSaved] = useState(false);
  const sentence = ruleSentence(t, locale, draft);
  const count = preview.data?.would_change ?? 0;
  const complete = preview.data?.complete ?? true;
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  const day = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" });
  const samples: RulePreviewSample[] = (preview.data?.samples ?? [])
    .slice(0, EXAMPLES)
    .map((sample) => ({
      subject: sample.subject,
      sender: sample.sender,
      when: day.format(new Date(sample.sent_at)),
    }));
  let summary: string | undefined;
  if (preview.isError) {
    summary = t("smartRule.matchesUnknown");
  } else if (preview.data !== undefined) {
    summary =
      count === 0
        ? t("smartRule.noMatches")
        : complete
          ? t("smartRule.matches", { count, n: number(count) })
          : t("smartRule.matchesAtLeast", { n: number(count) });
  }

  const save = () => {
    if (create.isPending || saved) {
      return;
    }
    setFailed(false);
    // A promise, not mutate's callbacks: closed while saving, the rule is
    // still made, so its Undo must still be offered.
    create
      .mutateAsync({
        inboxId: draft.inboxId,
        conditions: draft.conditions,
        actions: draft.actions,
        applyToExisting: existing && count > 0,
        origin: draft.origin,
        fromConversation: draft.fromConversation,
        noticeId: draft.noticeId,
      })
      .then(
        ({ rule }) => {
          setSaved(true);
          onSaved?.(rule);
          undo.push({
            label: t("smartRule.toast", { rule: sentence }),
            // A promise, not mutate's callbacks: the dialog is gone by the
            // time Undo is pressed, and a failure must still be said.
            undo: () => {
              // Undo all on the run already removed it: never twice.
              if (wasRemoved(rule.id)) {
                return;
              }
              remove
                .mutateAsync({ ruleId: rule.id, restore: true })
                .then(() => {
                  // Undo of the Undo brings the same rule back, applied again.
                  undo.push({
                    label: t("rules.toast.stopped", { rule: sentence }),
                    undo: () => {
                      restore.mutateAsync({ ruleId: rule.id, applyToExisting: true }).catch(() => {
                        undo.notify(t("rules.toast.failed"));
                      });
                    },
                  });
                })
                .catch(() => {
                  undo.notify(t("rules.run.undoFailed"));
                });
            },
          });
        },
        () => {
          setFailed(true);
        },
      );
  };
  useShortcut("inbox.rule.save", save, { enabled: !saved });

  return (
    <Dialog
      open
      onClose={onClose}
      title={draft.title}
      description={t("smartRule.lead")}
      result={
        saved ? { title: t("smartRule.saved"), detail: t("smartRule.savedDetail") } : undefined
      }
      onResultEnd={onClose}
      footer={
        <>
          <Button
            variant="primary"
            shortcut="inbox.rule.save"
            disabled={create.isPending}
            onClick={save}
          >
            {failed ? t("smartRule.retry") : t("smartRule.save")}
          </Button>
          <Button variant="ghost" shortcut="overlay.close" onClick={onClose}>
            {t("smartRule.cancel")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <RulePreview
          tag={t("smartRule.tag")}
          sentence={sentence}
          summary={summary}
          samples={samples}
          samplesLabel={t("smartRule.examples")}
          pending={preview.isPending}
        />
        <Text tone="secondary" size="sm">
          {t("smartRule.how")}
        </Text>
        {draft.sharedInbox === undefined ? null : (
          <Text tone="secondary" size="sm">
            {t("smartRule.shared", { inbox: draft.sharedInbox })}
          </Text>
        )}
        {count > 0 ? (
          <Checkbox
            label={t("smartRule.existing", { count, n: number(count) })}
            checked={existing}
            onChange={(event) => {
              setExisting(event.target.checked);
            }}
          />
        ) : null}
        {failed ? <Banner tone="danger" role="alert" title={t("smartRule.failed")} /> : null}
      </div>
    </Dialog>
  );
}
