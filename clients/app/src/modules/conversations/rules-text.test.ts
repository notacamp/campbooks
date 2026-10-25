import { describe, expect, it } from "vitest";

import { LEDGER_RULE, LEDGER_SUGGESTIONS } from "../../../test/demo/conversations";

import type { RuleAction } from "./api/rules";
import type { Translate } from "./model";
import { likenessTitle, originLine, ruleLine, ruleSentence, taughtKind } from "./rules-text";

/** A `t` that shows the key and its options, to test what is asked for. */
const t: Translate = (key, options) =>
  options === undefined ? key : `${key}${JSON.stringify(options)}`;

describe("rules in words", () => {
  it("says a rule as one sentence of its conditions and actions", () => {
    expect(
      ruleSentence(t, "en", {
        conditions: [
          { field: "sender_address", value: "notifications@github.com" },
          { field: "subject_contains", value: "pull request" },
        ],
        actions: [{ action: "move_to_stream", stream: "updates" }, { action: "mark_read" }],
      }),
    ).toBe(
      String.raw`rules.sentence{"conditions":"rules.when.from{\"value\":\"notifications@github.com\"} and rules.when.about{\"value\":\"pull request\"}","actions":"streams.updates and rules.do.read"}`,
    );
  });

  it("names a list by its readable part", () => {
    expect(
      ruleSentence(t, "en", {
        conditions: [{ field: "list_id", value: "acme/web <web.acme.github.com>" }],
        actions: [{ action: "archive" }],
      }),
    ).toContain(String.raw`rules.when.list{\"value\":\"acme/web\"}`);
  });

  it("titles a suggestion by its likeness", () => {
    const [sender] = LEDGER_SUGGESTIONS.suggestions;
    expect(sender === undefined ? "" : likenessTitle(t, sender)).toBe(
      'rules.likeness.sender{"address":"hello@weekly-ledger.example"}',
    );
  });

  it("titles the model's words for a sender's mail, and says a kind's template", () => {
    const [base] = LEDGER_SUGGESTIONS.suggestions;
    const phrase =
      base === undefined
        ? undefined
        : {
            ...base,
            likeness: "sender_phrase" as const,
            conditions: [
              { field: "sender_address" as const, value: "orders@shop.example" },
              { field: "subject_contains" as const, value: "sale starts now" },
            ],
          };
    expect(phrase === undefined ? "" : likenessTitle(t, phrase)).toBe(
      'rules.likeness.phrase{"phrase":"sale starts now","from":"orders@shop.example"}',
    );
    expect(
      ruleSentence(t, "en", {
        conditions: [
          { field: "sender_address", value: "orders@shop.example" },
          { field: "subject_template", value: "deals of the week #" },
        ],
        actions: [{ action: "archive" }],
      }),
    ).toContain(String.raw`rules.when.like{\"value\":\"deals of the week #\"}`);
  });

  it("says what a rule did to a row, counting the others", () => {
    expect(ruleLine(t, LEDGER_RULE, "en")).toBe(
      String.raw`rules.row.withOthers{"line":"rules.row.move","others":"rules.row.others{\"count\":39,\"n\":\"39\",\"from\":\"rules.when.from{\\\"value\\\":\\\"hello@weekly-ledger.example\\\"}\"}"}`,
    );
    expect(taughtKind({ kind: "not_needed" })).toBe("clear");
  });

  it("leaves out the others when there are none", () => {
    expect(ruleLine(t, { ...LEDGER_RULE, applied_count: 1 }, "en")).toBe("rules.row.move");
  });

  it("says each action with its own verb", () => {
    const line = (action: RuleAction) =>
      ruleLine(t, { ...LEDGER_RULE, applied_count: 0, actions: [action] }, "en");
    expect(line({ action: "add_label", label: "l" })).toBe("rules.row.label");
    expect(line({ action: "needs_reply", value: true })).toBe("rules.row.needsReply");
    expect(line({ action: "needs_reply", value: false })).toBe("rules.row.noReply");
    expect(line({ action: "asks_action", value: false })).toBe("rules.row.noAction");
    expect(line({ action: "surface", value: "always" })).toBe("rules.row.always");
    expect(line({ action: "surface", value: "never" })).toBe("rules.row.never");
    expect(line({ action: "mark_read" })).toBe("rules.row.read");
  });

  it("says who taught a rule", () => {
    const named = { kind: "named", name: "Marta" } as const;
    expect(originLine(t, LEDGER_RULE, { kind: "you" })).toBe(
      'rules.origin.action.you{"name":"","text":""}',
    );
    expect(originLine(t, { ...LEDGER_RULE, origin: "correction" }, named)).toBe(
      'rules.origin.correction.named{"name":"Marta","text":""}',
    );
    expect(originLine(t, { ...LEDGER_RULE, origin: "suggestion" }, named)).toBe(
      "rules.origin.suggestion",
    );
  });
});
