// What the assistant's screens decide without the server: how a rule reads
// as a sentence, which rules the answers to the questions become, and where
// a rule can be taught. Pure functions, so they are tested on their own.
import type { Action, Condition, Proposal, RuleShape } from "./api/queries";

/** A translator for dynamic keys of the `assistant` namespace. */
export type Translate = (key: string, options?: Record<string, unknown>) => string;

/** An inbox as the questions see it (from `conversations.list_inboxes`). */
export interface TeachInbox {
  id: string;
  address: string;
  role?: "reader" | "sender" | "manager" | null | undefined;
  shared: boolean;
}

/** Whether the member may teach rules there (`sender` or `manager`). */
export function teachable(inbox: TeachInbox): boolean {
  return inbox.role === "sender" || inbox.role === "manager";
}

/** Free mail providers: a rule for a whole domain of theirs would match strangers (as the server says). */
const FREE_MAIL = new Set([
  "gmail.com",
  "googlemail.com",
  "outlook.com",
  "hotmail.com",
  "live.com",
  "msn.com",
  "yahoo.com",
  "ymail.com",
  "icloud.com",
  "me.com",
  "mac.com",
  "aol.com",
  "proton.me",
  "protonmail.com",
  "gmx.com",
  "gmx.net",
  "zoho.com",
  "sapo.pt",
  "hotmail.fr",
  "orange.fr",
  "yahoo.es",
  "mail.com",
]);

/** A sender the member typed: an address, a domain, or why not. */
export type TypedSender =
  | { kind: "address"; value: string }
  | { kind: "domain"; value: string }
  | { kind: "invalid" }
  | { kind: "free_mail"; value: string };

/** Read what the member typed as a sender. */
export function readSender(raw: string): TypedSender {
  const value = raw.trim().toLowerCase().replace(/^@/u, "");
  if (/^[^\s@]+@[^\s@]+\.[a-z]{2,}$/u.test(value)) {
    return { kind: "address", value };
  }
  if (/^[a-z0-9-]+(\.[a-z0-9-]+)*\.[a-z]{2,}$/u.test(value)) {
    return FREE_MAIL.has(value) ? { kind: "free_mail", value } : { kind: "domain", value };
  }
  return { kind: "invalid" };
}

/** The condition naming a sender: their address, or anyone at a domain. */
function senderCondition(sender: string): Condition {
  return sender.includes("@")
    ? { field: "sender_address", value: sender }
    : { field: "sender_domain", value: sender };
}

/** The categories "what never needs you" offers. */
type NeverKind = "lists" | "automatic";

/** "This header is present". */
function headerPresent(name: "list_id" | "auto_submitted"): Condition {
  return { field: "header", value: { name } };
}

/** What the member answered. */
export interface Answers {
  /** Addresses or domains whose mail always stays in the inbox. */
  always: string[];
  /** Kinds of mail that never need them. */
  never: NeverKind[];
  /** Addresses or domains that never need them. */
  neverSenders: string[];
}

/** One rule an answer becomes, with a stable key for the screens. */
export interface AnswerRule extends RuleShape {
  key: string;
}

/** The rules the answers become, in the order they were asked. */
export function answerRules(answers: Answers): AnswerRule[] {
  const always: Action[] = [{ action: "surface", value: "always" }];
  const never: Action[] = [{ action: "surface", value: "never" }];
  return [
    ...answers.always.map((sender) => ({
      key: `always:${sender}`,
      conditions: [senderCondition(sender)],
      actions: always,
    })),
    ...answers.never.map((kind) => ({
      key: `never:${kind}`,
      conditions: [headerPresent(kind === "lists" ? "list_id" : "auto_submitted")],
      actions: never,
    })),
    ...answers.neverSenders.map((sender) => ({
      key: `never:${sender}`,
      conditions: [senderCondition(sender)],
      actions: never,
    })),
  ];
}

/** A proposal from compile with the `who` answered: the sender added as a condition. */
export function answerWho(proposal: Proposal, sender: string): RuleShape {
  return {
    conditions: [...proposal.conditions, senderCondition(sender)],
    actions: proposal.actions,
  };
}

/** Names to show next to addresses ("Rita Gomes (rita@example.com)"). */
export type Names = ReadonlyMap<string, string>;

function who(condition: Condition, t: Translate, names: Names): string | undefined {
  if (condition.field === "sender_address") {
    const name = names.get(condition.value);
    return name === undefined ? condition.value : `${name} (${condition.value})`;
  }
  if (condition.field === "sender_domain") {
    return t("rule.anyoneAt", { domain: condition.value });
  }
  return undefined;
}

function conditionPhrase(condition: Condition, t: Translate, labels: Names): string {
  switch (condition.field) {
    case "header": {
      return t(`rule.cond.header.${condition.value.name}`);
    }
    case "label": {
      return t("rule.cond.label", { value: labels.get(condition.value) ?? t("rule.aLabel") });
    }
    case "stream": {
      return t("rule.cond.stream", { value: t(`rule.stream.${condition.value}`) });
    }
    case "sender_checks_fail": {
      return t("rule.cond.sender_checks_fail");
    }
    default: {
      return t(`rule.cond.${condition.field}`, { value: condition.value });
    }
  }
}

function actionPhrase(action: Action, t: Translate, labels: Names): string {
  switch (action.action) {
    case "move_to_stream": {
      return t("rule.act.move_to_stream", { value: t(`rule.stream.${action.stream}`) });
    }
    case "add_label": {
      return t("rule.act.add_label", { value: labels.get(action.label) ?? t("rule.aLabel") });
    }
    case "needs_reply":
    case "asks_action": {
      return t(`rule.act.${action.action}_${String(action.value)}`);
    }
    case "surface": {
      return t(`rule.act.surface_${action.value}`);
    }
    default: {
      return t(`rule.act.${action.action}`);
    }
  }
}

/** "a, b and c" in the UI's language. */
function list(items: string[], locale: string): string {
  return new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(items);
}

/**
 * A rule as one sentence in the member's language: the common shapes in
 * plain words ("Mail from Rita Gomes always stays in your inbox…"), any
 * other as "When …: …".
 */
export function ruleSentence(
  rule: RuleShape,
  t: Translate,
  locale: string,
  names: Names = new Map(),
  labels: Names = new Map(),
): string {
  const [only] = rule.conditions;
  const [action] = rule.actions;
  if (rule.conditions.length === 1 && rule.actions.length === 1 && only !== undefined) {
    const sender = who(only, t, names);
    if (action?.action === "surface") {
      if (sender !== undefined) {
        return t(action.value === "always" ? "rule.alwaysFrom" : "rule.neverFrom", { who: sender });
      }
      if (action.value === "never" && only.field === "header") {
        if (only.value.name === "list_id") {
          return t("rule.neverLists");
        }
        if (only.value.name === "auto_submitted") {
          return t("rule.neverAutomatic");
        }
      }
    }
  }
  return t("rule.when", {
    conditions: list(
      rule.conditions.map((condition) => conditionPhrase(condition, t, labels)),
      locale,
    ),
    actions: list(
      rule.actions.map((each) => actionPhrase(each, t, labels)),
      locale,
    ),
  });
}

/** A fresh id for the rules one answer or sentence makes across inboxes. */
export function teachingId(): string {
  return crypto.randomUUID();
}
