import { describe, expect, it } from "vitest";

import {
  ACME,
  ANA,
  JONAS,
  LUZ,
  MARTA,
  MARTA_DETAIL,
  PEDRO,
  PHISH,
  RUI,
} from "../../../test/demo/attention";
import { NOW } from "../../../test/demo/conversations";

import type { NeedView } from "./api/queries";
import type { Translate } from "./model";
import {
  awaitedOf,
  dueLabel,
  howRows,
  isStillNeeded,
  needChip,
  needSender,
  needSentence,
  needSignal,
  quoteAttribution,
} from "./needs";

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
    expect(needSentence(t, need(LUZ), "Luz Energia")).toBe('needs.line.pay{"name":"Luz Energia"}');
    expect(needSentence(t, need(ACME), "Acme Hosting")).toBe(
      'needs.line.checkPay{"name":"Acme Hosting"}',
    );
    expect(needSentence(t, { ...need(MARTA), trust: "check" }, "Inês")).toBe(
      'needs.line.checkReply{"name":"Inês"}',
    );
    expect(needSentence(t, need(PHISH), "x")).toBe("needs.line.warning");
  });

  it("asks again about an old ask, saying how long ago it came", () => {
    // Pedro's undated lease, three weeks on (campbooks-cloud#275).
    const old: NeedView = {
      ...need(PEDRO),
      reason: "review",
      trust: "check",
      question: "still_to_do",
      due_on: null,
      sent_at: "2026-09-22T10:12:00Z",
    };
    expect(needSentence(t, old, "Pedro Alves", { locale: "en", now: NOW })).toBe(
      'needs.line.stillDo{"name":"Pedro Alves","ago":"3 weeks ago"}',
    );
    expect(needSentence(t, old, "Pedro Alves", { locale: "pt-PT", now: NOW })).toBe(
      'needs.line.stillDo{"name":"Pedro Alves","ago":"há 3 semanas"}',
    );
    const reply: NeedView = { ...old, question: "still_needs_reply" };
    expect(needSentence(t, reply, "Marta Ferreira", { locale: "fr", now: NOW })).toBe(
      'needs.line.stillReply{"name":"Marta Ferreira","ago":"il y a 3 semaines"}',
    );
    // Without its time, the plain question.
    expect(needSentence(t, { ...old, sent_at: null }, "Pedro Alves")).toBe(
      'needs.line.checkDo{"name":"Pedro Alves"}',
    );
    expect(needSentence(t, reply, "Marta Ferreira")).toBe(
      'needs.line.checkReply{"name":"Marta Ferreira"}',
    );
    // A question, in Review's colour.
    expect(needChip(t, old)).toBe("needs.chip.check");
    expect(needSignal(old)).toBe("review");
    expect(isStillNeeded(old) && isStillNeeded(reply)).toBe(true);
    expect(isStillNeeded(need(PEDRO))).toBe(false);
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
    // A bill still to pay says how late it is; anything else, since when.
    expect(dueLabel(t, need(LUZ), "en-GB", today)).toEqual({
      text: 'needs.due.daysOverdue{"count":1}',
      overdue: true,
      soon: true,
    });
    expect(dueLabel(t, { ...need(LUZ), reason: "do" }, "en-GB", today)?.text).toBe(
      'needs.due.overdue{"date":"Mon"}',
    );
    expect(dueLabel(t, need(ACME), "en-GB", today)?.text).toBe('needs.due.on{"date":"30 Oct"}');
    expect(dueLabel(t, need(JONAS), "en-GB", today)).toBeUndefined();
  });

  it("attributes the quote", () => {
    expect(quoteAttribution(need(MARTA), "Marta Ferreira", "en-GB")).toBe(
      "Marta Ferreira, Mon 12 Oct",
    );
    expect(NOW.getUTCFullYear()).toBe(2026);
  });

  it("says who has not written yet: someone else on the email, or the sender", () => {
    // João asked Ana: Ana Costa hasn't written yet.
    expect(needSentence(t, need(ANA), "Ana Costa", undefined, ANA)).toBe(
      'needs.line.nudge{"name":"Ana Costa"}',
    );
    expect(awaitedOf(need(ANA), ANA)).toEqual({
      name: "Ana Costa",
      address: "ana@costa-arquitetos.example",
      sender: false,
    });
    // Rui promised it: no word from Rui Matos yet, once it is time to nudge.
    const ruiNudge = { ...RUI, need: { ...need(RUI), reason: "nudge" as const } };
    expect(needSentence(t, need(ruiNudge), "Rui Matos", undefined, ruiNudge)).toBe(
      'needs.line.nudgeSender{"name":"Rui Matos"}',
    );
    // While Scout only waits, it says so.
    expect(needSentence(t, need(RUI), "Rui Matos", undefined, RUI)).toBe(
      'needs.line.waiting{"name":"Rui Matos"}',
    );
    expect(needSignal(need(ANA))).toBe("nudge");
    expect(needChip(t, need(RUI))).toBe('needs.chip.waiting{"name":"Rui Matos"}');
    // A wait's date is when Scout checks, never something due.
    expect(dueLabel(t, need(RUI), "en-GB", NOW)).toBeUndefined();
    expect(awaitedOf(need(MARTA), MARTA)).toBeUndefined();
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
