// What a taught rule says, in words: pure helpers over the API's records,
// shared by the offer, the rows and the rules list. Addresses, list names
// and subject words come from the member's own mail and are only ever
// interpolated into fixed sentences, as plain text.
import type { Stream } from "./api/queries";
import type {
  RuleAction,
  RuleCondition,
  RuleMark,
  RuleSuggestion,
  RuleView,
  TaughtAction,
} from "./api/rules";
import { streamName, type Translate } from "./model";

/** A List-Id's readable part: "acme/web" from "acme/web <web.acme.github.com>". */
function listName(listId: string): string {
  const open = listId.indexOf("<");
  const name = (open === -1 ? listId : listId.slice(0, open)).trim();
  if (name !== "") {
    return name;
  }
  return listId.replaceAll(/[<>]/gu, "").trim();
}

/** One condition, as a phrase that follows "Mail". */
function conditionPhrase(t: Translate, condition: RuleCondition): string {
  switch (condition.field) {
    case "sender_address":
    case "sender_domain": {
      return t("rules.when.from", { value: condition.value });
    }
    case "list_id": {
      return t("rules.when.list", { value: listName(condition.value) });
    }
    case "subject_template": {
      return t("rules.when.like", { value: condition.value });
    }
    case "subject_contains": {
      return t("rules.when.about", { value: condition.value });
    }
    case "header": {
      return t("rules.when.header");
    }
    case "label": {
      return t("rules.when.label");
    }
    case "stream": {
      return t("rules.when.stream", { value: streamName(t, condition.value) });
    }
    case "sender_checks_fail": {
      return t("rules.when.checksFail");
    }
  }
}

/**
 * The conditions as phrases; a domain with failed sender checks reads as one
 * ("that says it's from paypal.com but fails its sender checks").
 */
function conditionPhrases(t: Translate, conditions: readonly RuleCondition[]): string[] {
  const failing = conditions.some((condition) => condition.field === "sender_checks_fail");
  const domain = conditions.find((condition) => condition.field === "sender_domain");
  if (failing && domain?.field === "sender_domain") {
    return [
      t("rules.when.pretending", { value: domain.value }),
      ...conditions
        .filter((condition) => condition !== domain && condition.field !== "sender_checks_fail")
        .map((condition) => conditionPhrase(t, condition)),
    ];
  }
  return conditions.map((condition) => conditionPhrase(t, condition));
}

/** One action, as what happens to that mail. */
function actionPhrase(t: Translate, action: RuleAction): string {
  switch (action.action) {
    case "move_to_stream": {
      return streamName(t, action.stream);
    }
    case "archive": {
      return t("rules.do.archive");
    }
    case "add_label": {
      return t("rules.do.label");
    }
    case "mark_read": {
      return t("rules.do.read");
    }
    case "needs_reply": {
      return t(action.value ? "rules.do.needsReply" : "rules.do.noReply");
    }
    case "asks_action": {
      return t(action.value ? "rules.do.asksAction" : "rules.do.noAction");
    }
    case "surface": {
      return t(action.value === "always" ? "rules.do.always" : "rules.do.never");
    }
    case "trash": {
      return t("rules.do.trash");
    }
    case "spam": {
      return t("rules.do.spam");
    }
    case "mark_paid": {
      return t("rules.do.paid");
    }
  }
}

/** "a, b and c" in the reader's language. */
function joined(items: readonly string[], locale: string): string {
  return new Intl.ListFormat(locale, { type: "conjunction" }).format(items);
}

/** The whole rule as one sentence: "Mail from x about “y” → Updates". */
export function ruleSentence(
  t: Translate,
  locale: string,
  rule: { conditions: readonly RuleCondition[]; actions: readonly RuleAction[] },
): string {
  return t("rules.sentence", {
    conditions: joined(conditionPhrases(t, rule.conditions), locale),
    actions: joined(
      rule.actions.map((action) => actionPhrase(t, action)),
      locale,
    ),
  });
}

/** A suggestion's likeness, as its choice says it. */
export function likenessTitle(t: Translate, suggestion: RuleSuggestion): string {
  const value = (field: RuleCondition["field"]) => {
    const found = suggestion.conditions.find((condition) => condition.field === field);
    return found === undefined || typeof found.value !== "string" ? "" : found.value;
  };
  switch (suggestion.likeness) {
    case "sender": {
      return t("rules.likeness.sender", { address: value("sender_address") });
    }
    case "list": {
      return t("rules.likeness.list", { list: listName(value("list_id")) });
    }
    case "sender_template": {
      return t("rules.likeness.template", {
        template: value("subject_template"),
        from: value("sender_address"),
      });
    }
    case "domain_template": {
      return t("rules.likeness.template", {
        template: value("subject_template"),
        from: value("sender_domain"),
      });
    }
    case "domain": {
      return t("rules.likeness.domain", { domain: value("sender_domain") });
    }
    case "impersonation": {
      return t("rules.likeness.impersonation", { domain: value("sender_domain") });
    }
  }
}

/** The short "from …" a row or a done line uses for a rule. */
function likenessShort(t: Translate, conditions: readonly RuleCondition[]): string {
  return conditionPhrases(t, conditions)[0] ?? "";
}

/** The key of what was taught: "move", "archive", "read" or "clear". */
export function taughtKind(action: TaughtAction): "move" | "archive" | "read" | "clear" {
  switch (action.kind) {
    case "move_to_stream": {
      return "move";
    }
    case "archive": {
      return "archive";
    }
    case "mark_read": {
      return "read";
    }
    case "not_needed": {
      return "clear";
    }
  }
}

/** The stream a taught move goes to, for the sentences that name it. */
export function taughtStream(t: Translate, action: TaughtAction): string {
  return action.kind === "move_to_stream" ? streamName(t, action.stream) : "";
}

/** What a rule did to a conversation, said on its row: the first action's line. */
function markKey(actions: readonly RuleAction[]): string {
  const [first] = actions;
  switch (first?.action) {
    case undefined:
    case "move_to_stream": {
      return "move";
    }
    case "archive": {
      return "archive";
    }
    case "mark_read": {
      return "read";
    }
    case "add_label": {
      return "label";
    }
    case "needs_reply": {
      return first.value ? "needsReply" : "noReply";
    }
    case "asks_action": {
      return first.value ? "asksAction" : "noAction";
    }
    case "surface": {
      return first.value === "always" ? "always" : "never";
    }
    case "trash": {
      return "trash";
    }
    case "spam": {
      return "spam";
    }
    case "mark_paid": {
      return "paid";
    }
  }
}

/** Who taught a rule, as the rules list says it. */
export type RuleAuthor = { kind: "you" } | { kind: "named"; name: string } | { kind: "someone" };

/** Where a rule came from and who taught it, for the rules list. */
export function originLine(t: Translate, rule: RuleView, author: RuleAuthor): string {
  if (rule.origin === "suggestion") {
    return t("rules.origin.suggestion");
  }
  const options = {
    name: author.kind === "named" ? author.name : "",
    text: rule.origin_text ?? "",
  };
  return t(`rules.origin.${rule.origin}.${author.kind}`, options);
}

/** The stream a rule moves mail to, if it moves mail. */
export function ruleStream(actions: readonly RuleAction[]): Stream | undefined {
  const move = actions.find((action) => action.action === "move_to_stream");
  return move?.action === "move_to_stream" ? move.stream : undefined;
}

/** A handled row's line: "Campbooks moved this · like 1,000 others from …". */
export function ruleLine(t: Translate, mark: RuleMark, locale: string): string {
  const line = t(`rules.row.${markKey(mark.actions)}`);
  const others = Math.max(0, mark.applied_count - 1);
  if (others === 0) {
    return line;
  }
  return t("rules.row.withOthers", {
    line,
    others: t("rules.row.others", {
      count: others,
      n: new Intl.NumberFormat(locale).format(others),
      from: likenessShort(t, mark.conditions),
    }),
  });
}
