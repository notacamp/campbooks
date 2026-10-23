import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Banner, Button, Select, SkeletonList, Text, TextField } from "~/lib/ui";
import { useInboxes } from "~/modules/conversations";

import type { Action, Condition, RuleShape, Sender } from "../../api/queries";
import { readSender, teachable, type Translate } from "../../model";
import { RuleSave } from "../RuleSave/RuleSave";
import { SenderFilter } from "../SenderFilter/SenderFilter";

const CONDITIONS = ["from", "subject_contains", "list", "automatic"] as const;
type ConditionKind = (typeof CONDITIONS)[number];

const ACTIONS = ["always", "never", "updates", "newsletters", "archive"] as const;
type ActionKind = (typeof ACTIONS)[number];

const ACTION: Record<ActionKind, Action> = {
  always: { action: "surface", value: "always" },
  never: { action: "surface", value: "never" },
  updates: { action: "move_to_stream", stream: "updates" },
  newsletters: { action: "move_to_stream", stream: "newsletters" },
  archive: { action: "archive" },
};

/** A rule the builder made, with a stable key for its card. */
interface BuiltRule extends RuleShape {
  key: string;
}

/**
 * The condition naming one ticked sender (an address, or anyone at a
 * domain), or why not.
 */
function senderCondition(ticked: string, t: Translate): Condition | string {
  const sender = readSender(ticked);
  switch (sender.kind) {
    case "address": {
      return { field: "sender_address", value: sender.value };
    }
    case "domain": {
      return { field: "sender_domain", value: sender.value };
    }
    case "free_mail": {
      return t("flow.add.freeMail", { domain: sender.value });
    }
    default: {
      return t("flow.add.invalid");
    }
  }
}

/**
 * The rules the builder's fields say, or why not. Conditions all have to
 * hold, so several senders are several rules: mail from any of them counts.
 */
function builtRules(
  kind: ConditionKind,
  senders: readonly string[],
  words: string,
  action: ActionKind,
  t: Translate,
): BuiltRule[] | string {
  const actions = [ACTION[action]];
  switch (kind) {
    case "list": {
      return [
        { key: "list", conditions: [{ field: "header", value: { name: "list_id" } }], actions },
      ];
    }
    case "automatic": {
      return [
        {
          key: "automatic",
          conditions: [{ field: "header", value: { name: "auto_submitted" } }],
          actions,
        },
      ];
    }
    case "subject_contains": {
      const trimmed = words.trim();
      return trimmed.length === 0
        ? t("flow.add.invalid")
        : [
            {
              key: "subject",
              conditions: [{ field: "subject_contains", value: trimmed }],
              actions,
            },
          ];
    }
    default: {
      if (senders.length === 0) {
        return t("memory.builder.pickSender");
      }
      const rules: BuiltRule[] = [];
      for (const each of senders) {
        const condition = senderCondition(each, t);
        if (typeof condition === "string") {
          return condition;
        }
        rules.push({ key: `from:${each}`, conditions: [condition], actions });
      }
      return rules;
    }
  }
}

/**
 * A rule built by hand, with no language model: which mail (senders found by
 * name, as in Scout's questions, words in the subject, a mailing list or an
 * automatic notification) and what to do with it. Shows one rule card per
 * rule, each with what it matches, before Save; once every card is saved or
 * cancelled the builder starts again.
 */
export function RuleBuilder({
  workspaceId,
  onSaved,
}: {
  workspaceId: string;
  /** A rule was saved (Teach Scout opened from Scout's chat settles its card). */
  onSaved?: (() => void) | undefined;
}) {
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const inboxes = useInboxes(workspaceId);
  const [kind, setKind] = useState<ConditionKind>("from");
  const [senders, setSenders] = useState<string[]>([]);
  // Senders ticked from the mail, so their cards and rules keep their names.
  const [found, setFound] = useState<ReadonlyMap<string, Sender>>(new Map());
  const [words, setWords] = useState("");
  const [action, setAction] = useState<ActionKind>("always");
  const [error, setError] = useState<string | undefined>();
  const [rules, setRules] = useState<readonly BuiltRule[]>([]);
  // The cards saved or cancelled; once every one is, the form is back.
  const [closed, setClosed] = useState<ReadonlySet<string>>(new Set());
  const open = rules.filter((rule) => !closed.has(rule.key));
  const targets = (inboxes.data ?? []).filter(teachable).map((inbox) => ({
    id: inbox.id,
    address: inbox.address,
    shared: inbox.shared,
  }));
  const names = new Map(
    [...found.values()].flatMap((sender) =>
      sender.name === null || sender.name === undefined
        ? []
        : [[sender.address, sender.name] as const],
    ),
  );

  /** A card is saved or cancelled: the rest stay. */
  const done = (key: string) => {
    setClosed((current) => new Set(current).add(key));
  };

  if (open.length > 0 && inboxes.isPending) {
    // Not "read-only": the inboxes aren't known yet.
    return <SkeletonList rows={1} />;
  }
  if (open.length > 0 && inboxes.isError) {
    return (
      <Banner
        tone="danger"
        role="alert"
        title={t("memory.teach.inboxesFailed")}
        action={
          <Button
            variant="secondary"
            onClick={() => {
              void inboxes.refetch();
            }}
          >
            {t("memory.teach.tryAgain")}
          </Button>
        }
      />
    );
  }
  if (open.length > 0) {
    return (
      <div className="flex flex-col gap-6">
        {rules.length > 1 ? <Text tone="secondary">{t("memory.builder.oneEach")}</Text> : null}
        {open.map((rule, index) => (
          <RuleSave
            key={rule.key}
            workspaceId={workspaceId}
            rule={rule}
            inboxes={targets}
            origin="instruction"
            names={names}
            primary={index === 0}
            onSaved={onSaved}
            onDone={() => {
              done(rule.key);
            }}
          />
        ))}
      </div>
    );
  }
  const many = kind === "from" && senders.length > 1;
  return (
    <form
      className="flex flex-col gap-3"
      aria-label={t("memory.builder.title")}
      onSubmit={(event) => {
        event.preventDefault();
        const built = builtRules(kind, senders, words, action, t);
        if (typeof built === "string") {
          setError(built);
          return;
        }
        setError(undefined);
        // The form is hidden while the cards show, and empty when it is back.
        setRules(built);
        setClosed(new Set());
        setSenders([]);
        setWords("");
      }}
    >
      <Select
        label={`${t("memory.builder.when")} · ${t("memory.builder.condition")}`}
        value={kind}
        onChange={(next) => {
          setKind(next);
          setError(undefined);
        }}
        options={CONDITIONS.map((each) => ({
          value: each,
          label: t(`memory.builder.conditions.${each}`),
        }))}
      />
      {kind === "from" ? (
        <SenderFilter
          workspaceId={workspaceId}
          label={t("memory.builder.senders")}
          known={found}
          value={senders}
          onChange={(next, ticked) => {
            setSenders(next);
            setError(undefined);
            if (ticked !== undefined) {
              setFound((current) => new Map(current).set(ticked.address, ticked));
            }
          }}
        />
      ) : null}
      {kind === "subject_contains" ? (
        <TextField
          label={t("memory.builder.words")}
          value={words}
          error={error}
          onChange={(event) => {
            setWords(event.target.value);
          }}
        />
      ) : null}
      {kind !== "subject_contains" && error !== undefined ? (
        <div role="alert">
          <Text tone="danger" size="sm">
            {error}
          </Text>
        </div>
      ) : null}
      <Select
        label={`${t("memory.builder.then")} · ${t("memory.builder.action")}`}
        value={action}
        onChange={setAction}
        options={ACTIONS.map((each) => ({
          value: each,
          label: t(`memory.builder.actions.${each}`),
        }))}
      />
      <div className="flex flex-wrap gap-2">
        <Button type="submit" variant="primary" keys="enter">
          {many ? t("memory.builder.showMany") : t("memory.builder.show")}
        </Button>
      </div>
    </form>
  );
}
