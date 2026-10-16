import { describe, expect, it } from "vitest";

import { ACME, JONAS, LUZ, MARTA, MARTA_DETAIL, PHISH } from "../../../test/demo/attention";
import { NOW } from "../../../test/demo/conversations";

import type { NeedView } from "./api/queries";
import type { Translate } from "./model";
import { dueLabel, howRows, needChip, needSender, needSentence, quoteAttribution } from "./needs";

/** A `t` that shows the key and its options, to test what is asked for. */
const t: Translate = (key, options) =>
  options === undefined ? key : `${key}${JSON.stringify(options)}`;
const need = (summary: { need?: NeedView | null }): NeedView => {
  if (summary.need === null || summary.need === undefined) {
    throw new Error("no need");
  }
  return summary.need;
};

describe("needs in words", () => {
  it("chooses the sentence by reason and rule, naming the sender", () => {
    expect(needSentence(t, need(MARTA), "Marta Ferreira")).toBe(
      'needs.line.reply{"name":"Marta Ferreira"}',
    );
    expect(needSentence(t, need(JONAS), "Jonas Weber")).toBe(
      'needs.line.question{"name":"Jonas Weber"}',
    );
    expect(needSentence(t, need(ACME), "Acme Hosting")).toBe(
      'needs.line.pay{"name":"Acme Hosting"}',
    );
    expect(needSentence(t, { ...need(MARTA), trust: "check" }, "Inês")).toBe(
      'needs.line.checkReply{"name":"Inês"}',
    );
    expect(needSentence(t, need(PHISH), "x")).toBe("needs.line.warning");
  });

  it("asks whether a bill is paid, in Pay's colour", () => {
    const bill: NeedView = { ...need(ACME), trust: "check", question: "money" };
    expect(needSentence(t, bill, "Acme Hosting")).toBe(
      'needs.line.checkPay{"name":"Acme Hosting"}',
    );
    expect(needChip(t, bill)).toBe("needs.chip.pay");
  });

  it("says the money answer in words", () => {
    const base = MARTA_DETAIL.decisions[0];
    if (base === undefined) {
      throw new Error("no decision");
    }
    const [money] = howRows(t, () => true, [
      { ...base, question: "money", value: "owed", reasons: [] },
    ]);
    expect(money?.question).toBe("scout.whyMoney");
    expect(money?.answer).toBe("needs.how.money.owed");
  });

  it("says a need comes from a rule, never that the sender asked", () => {
    expect(
      needSentence(
        t,
        { ...need(MARTA), basis: "taught", reason: "do", question: "asks_action" },
        "Marta Ferreira",
      ),
    ).toBe('needs.line.taughtDo{"name":"Marta Ferreira"}');
    expect(needSentence(t, { ...need(MARTA), basis: "taught" }, "Marta Ferreira")).toBe(
      'needs.line.taughtReply{"name":"Marta Ferreira"}',
    );
  });

  it("names a warning's sender by address, not by its display name", () => {
    expect(needSender(need(PHISH), PHISH)).toBe("it-support@at3lier-sol.example");
    expect(needSender(need(MARTA), MARTA)).toBe("Marta Ferreira");
    expect(needChip(t, need(PHISH))).toBe("needs.chip.suspicious");
    expect(needChip(t, need(MARTA))).toBe("needs.chip.reply");
  });

  it("says due dates relative to today", () => {
    const today = new Date(2026, 9, 13, 15);
    expect(dueLabel(t, need(MARTA), "en-GB", today)).toEqual({
      text: "needs.due.tomorrow",
      overdue: false,
      soon: true,
    });
    expect(dueLabel(t, need(LUZ), "en-GB", today)).toEqual({
      text: 'needs.due.overdue{"date":"Mon"}',
      overdue: true,
      soon: true,
    });
    expect(dueLabel(t, need(ACME), "en-GB", today)?.text).toBe('needs.due.on{"date":"30 Oct"}');
    expect(dueLabel(t, need(JONAS), "en-GB", today)).toBeUndefined();
  });

  it("attributes the quote", () => {
    expect(quoteAttribution(need(MARTA), "Marta Ferreira", "en-GB")).toBe(
      "Marta Ferreira, Mon 12 Oct",
    );
    expect(NOW.getUTCFullYear()).toBe(2026);
  });

  it("keeps the numbers to How Campbooks decided, in words first", () => {
    const rows = howRows(t, () => true, MARTA_DETAIL.decisions);
    expect(rows.map((row) => row.question)).toEqual([
      "scout.whyNeedsReply",
      "scout.whyHasDeadline",
      "scout.whySuspicious",
    ]);
    expect(rows[0]?.answer).toBe(
      'needs.how.yes reasons.signals.reply_asked{"evidence":"could you confirm"}',
    );
    expect(rows[0]?.source).toBe(
      'needs.how.source.rules · 0.90 · needs.how.actsFrom{"threshold":"0.88"}',
    );
    expect(rows[2]?.source).toBe("needs.how.source.rules · 0.99 · needs.how.suggestion");
  });
});
