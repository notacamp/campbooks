import { describe, expect, it } from "vitest";

import { LEDGER_RULE, LEDGER_SUGGESTIONS } from "../../../test/demo/conversations";

import type { Translate } from "./model";
import { likenessTitle, ruleLine, ruleSentence, taughtKind } from "./rules-text";

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
      'rules.sentence{"conditions":"rules.when.from{\\"value\\":\\"notifications@github.com\\"} and rules.when.about{\\"value\\":\\"pull request\\"}","actions":"streams.updates and rules.do.read"}',
    );
  });

  it("names a list by its readable part", () => {
    expect(
      ruleSentence(t, "en", {
        conditions: [{ field: "list_id", value: "acme/web <web.acme.github.com>" }],
        actions: [{ action: "archive" }],
      }),
    ).toContain('rules.when.list{\\"value\\":\\"acme/web\\"}');
  });

  it("titles a suggestion by its likeness", () => {
    const [sender] = LEDGER_SUGGESTIONS.suggestions;
    expect(sender === undefined ? "" : likenessTitle(t, sender)).toBe(
      'rules.likeness.sender{"address":"hello@weekly-ledger.example"}',
    );
  });

  it("says what a rule did to a row, counting the others", () => {
    expect(ruleLine(t, LEDGER_RULE, "en")).toBe(
      'rules.row.move{"count":"39","from":"rules.when.from{\\"value\\":\\"hello@weekly-ledger.example\\"}"}',
    );
    expect(taughtKind({ kind: "not_needed" })).toBe("clear");
  });
});
