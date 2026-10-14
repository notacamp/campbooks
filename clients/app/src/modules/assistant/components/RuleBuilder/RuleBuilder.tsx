import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Select, TextField } from "~/lib/ui";
import { useInboxes } from "~/modules/conversations";

import type { Action, Condition, RuleShape } from "../../api/queries";
import { readSender, teachable, type Translate } from "../../model";
import { RuleSave } from "../RuleSave/RuleSave";

const CONDITIONS = [
  "sender_address",
  "sender_domain",
  "subject_contains",
  "list",
  "automatic",
] as const;
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

/** The condition the builder's fields say, or why not. */
function condition(kind: ConditionKind, value: string, t: Translate): Condition | string {
  switch (kind) {
    case "list": {
      return { field: "header", value: { name: "list_id" } };
    }
    case "automatic": {
      return { field: "header", value: { name: "auto_submitted" } };
    }
    case "subject_contains": {
      const words = value.trim();
      return words.length === 0
        ? t("flow.add.invalid")
        : { field: "subject_contains", value: words };
    }
    default: {
      const sender = readSender(value);
      if (sender.kind === "free_mail") {
        return t("flow.add.freeMail", { domain: sender.value });
      }
      if (kind === "sender_address" && sender.kind === "address") {
        return { field: "sender_address", value: sender.value };
      }
      if (kind === "sender_domain" && sender.kind === "domain") {
        return { field: "sender_domain", value: sender.value };
      }
      return t("flow.add.invalid");
    }
  }
}

/**
 * A rule built by hand, for a server with no language model for sentences:
 * which mail, and what to do with it. Shows the same rule card, with what it
 * matches, before Save.
 */
export function RuleBuilder({ workspaceId }: { workspaceId: string }) {
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const inboxes = useInboxes(workspaceId);
  const [kind, setKind] = useState<ConditionKind>("sender_address");
  const [value, setValue] = useState("");
  const [action, setAction] = useState<ActionKind>("always");
  const [error, setError] = useState<string | undefined>();
  const [rule, setRule] = useState<RuleShape | undefined>();
  const needsValue = kind !== "list" && kind !== "automatic";
  const targets = (inboxes.data ?? []).filter(teachable).map((inbox) => ({
    id: inbox.id,
    address: inbox.address,
    shared: inbox.shared,
  }));

  if (rule !== undefined) {
    return (
      <RuleSave
        workspaceId={workspaceId}
        rule={rule}
        inboxes={targets}
        origin="instruction"
        focusSave
        onDone={() => {
          setRule(undefined);
          setValue("");
        }}
      />
    );
  }
  return (
    <form
      className="flex flex-col gap-3"
      aria-label={t("memory.teach.title")}
      onSubmit={(event) => {
        event.preventDefault();
        const built = condition(kind, value, t);
        if (typeof built === "string") {
          setError(built);
          return;
        }
        setError(undefined);
        setRule({ conditions: [built], actions: [ACTION[action]] });
      }}
    >
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <Select
          label={`${t("memory.builder.when")} · ${t("memory.builder.condition")}`}
          value={kind}
          onChange={setKind}
          options={CONDITIONS.map((each) => ({
            value: each,
            label: t(`memory.builder.conditions.${each}`),
          }))}
        />
        {needsValue ? (
          <TextField
            label={t("memory.builder.value")}
            value={value}
            error={error}
            onChange={(event) => {
              setValue(event.target.value);
            }}
          />
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
      </div>
      <div className="flex flex-wrap gap-2">
        <Button type="submit" variant="primary" keys="enter">
          {t("memory.builder.show")}
        </Button>
      </div>
    </form>
  );
}
