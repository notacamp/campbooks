import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Banner, Button, CheckCards, Text, useUndoToast } from "~/lib/ui";

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
  /** Put the focus on Save when shown, so Enter saves. */
  focusSave?: boolean | undefined;
}

/**
 * A rule to save: what it matches, where (an inbox shared with others only
 * when switched on, marked "for everyone"), whether it also applies to the
 * mail already there, then Save (Enter) or Cancel (Esc). Saving creates it
 * active in each chosen inbox, with one Undo.
 */
export function RuleSave({
  workspaceId,
  rule,
  inboxes,
  origin,
  originText,
  names,
  onDone,
  focusSave = false,
}: RuleSaveProps) {
  const { t: typed, i18n } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const create = useCreateRules(workspaceId);
  const remove = useRemoveRules(workspaceId);
  const undo = useUndoToast();
  const save = useRef<HTMLButtonElement>(null);
  const [sharedOn, setSharedOn] = useState<string[]>([]);
  const [existing, setExisting] = useState(true);
  const mine = inboxes.filter((inbox) => !inbox.shared);
  const shared = inboxes.filter((inbox) => inbox.shared);
  const targets = [...mine, ...shared.filter((inbox) => sharedOn.includes(inbox.id))];
  useEffect(() => {
    if (focusSave) {
      save.current?.focus();
    }
  }, [focusSave]);
  useShortcut("overlay.close", onDone);

  if (inboxes.length === 0) {
    return <Text tone="secondary">{t("memory.teach.readOnly")}</Text>;
  }
  return (
    <form
      className="flex flex-col gap-3"
      aria-label={ruleSentence(rule, t, i18n.language, names)}
      onSubmit={(event) => {
        event.preventDefault();
        if (create.isPending || targets.length === 0) {
          return;
        }
        const teaching = teachingId();
        create.mutate(
          targets.map((inbox) => ({
            inboxId: inbox.id,
            conditions: rule.conditions,
            actions: rule.actions,
            origin,
            originText,
            applyToExisting: existing,
            teachingId: teaching,
          })),
          {
            onSuccess: (ids) => {
              undo.push({
                label: t("memory.teach.saved", { count: ids.length }),
                undo: () => {
                  remove.mutate(ids);
                },
              });
              onDone();
            },
          },
        );
      }}
    >
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
      {create.isError ? (
        <Banner tone="danger" role="alert" title={t("flow.reviewStep.failed")} />
      ) : null}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          ref={save}
          type="submit"
          variant="primary"
          keys="enter"
          disabled={create.isPending || targets.length === 0}
        >
          {t("memory.teach.save")}
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
