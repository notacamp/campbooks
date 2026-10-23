import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Banner, Button, CheckCards, Text, useUndoToast, VisuallyHidden } from "~/lib/ui";

import { useCreateRules, useRemoveRules, type RuleOrigin, type RuleShape } from "../../api/queries";
import { ruleSentence, teachingId, type Names, type Translate } from "../../model";
import { RuleMatch } from "../RuleMatch/RuleMatch";

/** An inbox a rule may be saved in. */
interface SaveInbox {
  id: string;
  address: string;
  shared: boolean;
}

export interface RuleSaveProps {
  workspaceId: string;
  rule: RuleShape;
  /** Where the member may teach it; shared ones start off. */
  inboxes: readonly SaveInbox[];
  origin: RuleOrigin;
  /** The member's own words, kept with an instruction. */
  originText?: string | undefined;
  names?: Names | undefined;
  /** After saving (or Cancel). */
  onDone: () => void;
  /** Saved in every inbox chosen (before {@link onDone}); not on Cancel. */
  onSaved?: (() => void) | undefined;
  /**
   * The one shown first: ⌘↵ saves it while the focus stays where the member
   * was typing, and it is announced. Several rules shown at once each get a
   * Save; only one answers the keys.
   */
  primary?: boolean | undefined;
}

/**
 * A rule to save: what it matches, where (an inbox shared with others only
 * when switched on, marked "for everyone"), whether it also applies to the
 * mail already there, then Save (⌘↵) or Cancel (Esc). Saving creates it
 * active in each chosen inbox, with one Undo; when an inbox fails, the ones
 * saved stay saved and Save tries only the rest.
 */
export function RuleSave({
  workspaceId,
  rule,
  inboxes,
  origin,
  originText,
  names,
  onDone,
  onSaved,
  primary = false,
}: RuleSaveProps) {
  const { t: typed, i18n } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const create = useCreateRules(workspaceId);
  const remove = useRemoveRules(workspaceId);
  const undo = useUndoToast();
  const [sharedOn, setSharedOn] = useState<string[]>([]);
  const [existing, setExisting] = useState(true);
  // Inboxes it is already in (inbox id → rule id), so a retry sends only the rest.
  const [created, setCreated] = useState<ReadonlyMap<string, string>>(new Map());
  // One teaching for every inbox, the same across retries.
  const [teaching] = useState(teachingId);
  const [failed, setFailed] = useState(false);
  const mine = inboxes.filter((inbox) => !inbox.shared);
  const shared = inboxes.filter((inbox) => inbox.shared);
  const targets = [...mine, ...shared.filter((inbox) => sharedOn.includes(inbox.id))];
  const sentence = ruleSentence(rule, t, i18n.language, names);
  const save = useRef<HTMLButtonElement>(null);
  // Said once it is shown (a live region only speaks what changes in it).
  const [announced, setAnnounced] = useState(false);
  useEffect(() => {
    if (!primary) {
      return;
    }
    // The focus stays where the member was typing; only when nothing holds
    // it (the form they typed in is gone) does it come to Save.
    if (document.activeElement === null || document.activeElement === document.body) {
      save.current?.focus();
    }
    // Filled after the region is in the page, so it is spoken.
    const timer = setTimeout(() => {
      setAnnounced(true);
    }, 0);
    return () => {
      clearTimeout(timer);
    };
  }, [primary]);

  const submit = () => {
    if (create.isPending || targets.length === 0) {
      return;
    }
    const pending = targets.filter((inbox) => !created.has(inbox.id));
    create.mutate(
      pending.map((inbox) => ({
        key: inbox.id,
        inboxId: inbox.id,
        conditions: rule.conditions,
        actions: rule.actions,
        origin,
        originText,
        applyToExisting: existing,
        teachingId: teaching,
      })),
      {
        onSuccess: (result) => {
          const all = new Map(created);
          for (const each of result.created) {
            all.set(each.key, each.id);
          }
          setCreated(all);
          if (result.failed) {
            setFailed(true);
            return;
          }
          const ids = [...all.values()];
          undo.push({
            label: t("memory.teach.saved", { count: ids.length }),
            // A promise, not mutate's callbacks: this form is gone by the time
            // Undo is pressed, and the failure must still be said.
            undo: () => {
              remove.mutateAsync(ids).catch(() => {
                undo.notify(t("flow.undoFailed"));
              });
            },
          });
          onSaved?.();
          onDone();
        },
        onError: () => {
          setFailed(true);
        },
      },
    );
  };
  useShortcut("overlay.close", onDone);
  useShortcut("scout.save_rule", submit, { enabled: primary && inboxes.length > 0 });

  if (inboxes.length === 0) {
    return <Text tone="secondary">{t("memory.teach.readOnly")}</Text>;
  }
  return (
    <form
      className="flex flex-col gap-3"
      aria-label={sentence}
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      {primary ? (
        <p role="status">
          {announced ? (
            <VisuallyHidden>{t("memory.teach.announce", { rule: sentence })}</VisuallyHidden>
          ) : null}
        </p>
      ) : null}
      <RuleMatch
        workspaceId={workspaceId}
        rule={rule}
        inboxIds={targets.map((inbox) => inbox.id)}
        names={names}
      />
      {mine.length === 0 ? null : (
        <Text tone="secondary" size="sm">
          {t("memory.teach.where", { inboxes: mine.map((inbox) => inbox.address).join(", ") })}
        </Text>
      )}
      <CheckCards
        label={t("memory.teach.existing")}
        hideLabel
        value={[...sharedOn, ...(existing ? ["existing"] : [])]}
        onChange={(values) => {
          setSharedOn(values.filter((value) => value !== "existing"));
          setExisting(values.includes("existing"));
        }}
        options={[
          ...shared.map((inbox) => ({
            value: inbox.id,
            title: t("memory.teach.forEveryone", { address: inbox.address }),
          })),
          { value: "existing", title: t("memory.teach.existing") },
        ]}
      />
      {failed ? (
        <Banner
          tone="danger"
          role="alert"
          title={
            created.size === 0
              ? t("memory.teach.saveFailed")
              : t("memory.teach.savePartial", {
                  saved: created.size,
                  total: created.size + targets.filter((inbox) => !created.has(inbox.id)).length,
                })
          }
        />
      ) : null}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          ref={save}
          type="submit"
          variant="primary"
          shortcut={primary ? "scout.save_rule" : undefined}
          disabled={create.isPending || targets.length === 0}
        >
          {failed ? t("memory.teach.retry") : t("memory.teach.save")}
        </Button>
        <Button variant="ghost" shortcut="overlay.close" onClick={onDone}>
          {t("memory.teach.cancel")}
        </Button>
      </div>
      <Text tone="tertiary" size="sm">
        {t("memory.teach.runsWithoutAi")}
      </Text>
    </form>
  );
}
