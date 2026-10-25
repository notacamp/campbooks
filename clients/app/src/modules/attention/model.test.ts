import { describe, expect, it } from "vitest";

import type { ItemView } from "./api/queries";
import {
  ageLabel,
  doneTodayKind,
  groupByBand,
  reasonDay,
  reasonSentence,
  isPressing,
  remindTime,
  rowActionIds,
  rowChoices,
  signalOf,
  snoozeOptions,
} from "./model";

const item = (id: string, reason: ItemView["reason"], extra: Partial<ItemView> = {}): ItemView => ({
  id,
  record: { type: "conversations.conversation", id: `c-${id}` },
  container: { type: "conversations.inbox", id: "i" },
  cause: { type: "conversations.message", id: `m-${id}` },
  reason,
  trust: "sure",
  due_on: null,
  overdue: false,
  urgent: false,
  since: "2026-10-13T08:00:00Z",
  state: "open",
  cleared_because: null,
  cleared_by: null,
  cleared_at: null,
  snoozed_until: null,
  ...extra,
});

describe("attention model", () => {
  it("groups by band in Home's order, keeping the server's order inside", () => {
    const groups = groupByBand([
      item("a", "pay", { band: "week" }),
      item("b", "reply", { band: "today" }),
      item("c", "worth_a_look", { band: "whenever" }),
      item("d", "review", { band: "now" }),
      item("e", "do", { band: "week" }),
    ]);
    expect(groups.map((g) => g.band)).toEqual(["now", "today", "week", "whenever"]);
    expect(groups[2]?.items.map((i) => i.id)).toEqual(["a", "e"]);
  });

  it("puts an item without a band in Whenever, and never lists a wait", () => {
    const groups = groupByBand([item("a", "pay"), item("b", "waiting", { band: "week" })]);
    expect(groups).toEqual([{ band: "whenever", items: [expect.objectContaining({ id: "a" })] }]);
  });

  it("colours reasons by kind: Worth a look its own, a wait as a nudge", () => {
    expect(signalOf("worth_a_look")).toBe("look");
    expect(signalOf("pay")).toBe("pay");
    expect(signalOf("nudge")).toBe("nudge");
    expect(signalOf("waiting")).toBe("nudge");
  });

  it("says why an item is in its band, naming the person when there is one", () => {
    const today = new Date(2026, 9, 26, 10, 0);
    const say = (reason: Parameters<typeof reasonSentence>[0], name?: string) =>
      reasonSentence(reason, { name, today, locale: "en-GB" });
    expect(say({ rule: "warning" })).toEqual({ key: "reason.warning", values: {} });
    expect(say({ rule: "overdue", days: 2 })).toEqual({
      key: "reason.overdue",
      values: { count: 2 },
    });
    expect(say({ rule: "due_today" }).key).toBe("reason.dueToday");
    expect(say({ rule: "due_tomorrow" }).key).toBe("reason.dueTomorrow");
    expect(say({ rule: "planned_today" }).key).toBe("reason.plannedToday");
    expect(say({ rule: "waited", days: 4 }, "Pedro")).toEqual({
      key: "reason.waited",
      values: { name: "Pedro", count: 4 },
    });
    expect(say({ rule: "waited", days: 4 })).toEqual({
      key: "reason.waitedNoName",
      values: { count: 4 },
    });
    expect(say({ rule: "waiting_answer" }, "Marta")).toEqual({
      key: "reason.waitingAnswer",
      values: { name: "Marta" },
    });
    expect(say({ rule: "waiting_answer" }, "").key).toBe("reason.waitingAnswerNoName");
    expect(say({ rule: "due_on", on: "2026-10-28" })).toEqual({
      key: "reason.dueOn",
      values: { date: "Wed" },
    });
    expect(say({ rule: "nudge_due", on: "2026-10-23" }, "Rui")).toEqual({
      key: "reason.nudgeDue",
      values: { name: "Rui", date: "Fri" },
    });
    expect(say({ rule: "nudge_due", on: null }, "Rui").key).toBe("reason.nudgeDueToday");
    expect(say({ rule: "nudge_due", on: "2026-10-26" }).key).toBe("reason.nudgeDueTodayNoName");
    expect(say({ rule: "kept_by_rule" }).key).toBe("reason.keptByRule");
    expect(say({ rule: "no_date" }).key).toBe("reason.noDate");
    expect(say(null).key).toBe("reason.noDate");
  });

  it("names a day within the week by its weekday, further days by the month", () => {
    const today = new Date(2026, 9, 26, 10, 0);
    expect(reasonDay("2026-10-30", today, "en-GB")).toBe("Fri");
    expect(reasonDay("2026-10-20", today, "en-GB")).toBe("Tue");
    expect(reasonDay("2026-11-09", today, "en-GB")).toBe("9 Nov");
  });

  it("draws warnings and anything overdue in the danger colour", () => {
    expect(isPressing({ rule: "warning" })).toBe(true);
    expect(isPressing({ rule: "overdue", days: 1 })).toBe(true);
    expect(isPressing({ rule: "due_today" })).toBe(false);
    expect(isPressing(null)).toBe(false);
  });

  it("offers a nudge its own actions, whatever the catalogue rated", () => {
    expect(rowActionIds("nudge")).toEqual({
      own: ["nudge"],
      universal: ["done", "snooze", "not_needed"],
    });
    const choices = rowChoices("nudge", [{ actions: ["reply"], likelihood: 90 }]);
    expect(choices.own.map((c) => c.id)).toEqual(["nudge"]);
    expect(choices.picked).toBe(false);
  });

  it("says when the person Scout waited on wrote back", () => {
    expect(
      doneTodayKind(item("a", "waiting", { state: "cleared", cleared_because: "heard_back" })),
    ).toEqual({ key: "doneToday.heardBack", undo: undefined });
  });

  it("offers later today only while it is worth it, and next week", () => {
    // Tuesday 13 October 2026, 09:00 local.
    const morning = new Date(2026, 9, 13, 9, 0);
    const options = snoozeOptions(morning);
    expect(options.map((o) => o.key)).toEqual(["today", "tomorrow", "monday", "nextWeek"]);
    const monday = options.find((o) => o.key === "monday")?.until;
    expect(monday?.getDay()).toBe(1);
    expect(monday?.getDate()).toBe(19);
    const week = options.find((o) => o.key === "nextWeek")?.until;
    expect([week?.getDate(), week?.getHours()]).toEqual([20, 8]);
    const evening = new Date(2026, 9, 13, 17, 30);
    expect(snoozeOptions(evening).map((o) => o.key)).toEqual(["tomorrow", "monday", "nextWeek"]);
  });

  it("never offers the same day twice", () => {
    // On a Monday, Monday means next week: Next week would repeat it.
    const onMonday = snoozeOptions(new Date(2026, 9, 19, 9, 0));
    expect(onMonday.map((o) => o.key)).toEqual(["today", "tomorrow", "monday"]);
    expect(onMonday.at(-1)?.until.getDate()).toBe(26);
    // On a Sunday, Monday is tomorrow.
    const onSunday = snoozeOptions(new Date(2026, 9, 18, 19, 0));
    expect(onSunday.map((o) => o.key)).toEqual(["tomorrow", "nextWeek"]);
  });

  it("offers each kind of need its own actions, without repeating one", () => {
    expect(rowActionIds("need")).toEqual({
      own: ["reply"],
      universal: ["done", "snooze", "not_needed"],
    });
    expect(rowActionIds("question")).toEqual({ own: ["yes", "no"], universal: ["snooze"] });
    expect(rowActionIds("unsure")).toEqual({
      own: ["wait", "reply_to", "nothing"],
      universal: ["snooze"],
    });
    expect(rowActionIds("bill")).toEqual({
      own: ["paid", "to_pay"],
      universal: ["snooze", "not_needed"],
    });
    expect(rowActionIds("toPay")).toEqual({
      own: ["remind", "paid"],
      universal: ["snooze", "not_needed"],
    });
    expect(rowActionIds("warning")).toEqual({
      own: ["report", "safe"],
      universal: ["done", "snooze"],
    });
    // Worth a look asks nothing: Seen (Done) and Snooze, never Not needed.
    expect(rowActionIds("highlight")).toEqual({ own: [], universal: ["done", "snooze"] });
    // Whatever the server ranked, it offers no reply.
    expect(rowChoices("highlight", [{ actions: ["reply"], likelihood: 90 }]).own).toEqual([]);
  });

  it("offers the fixed actions when the server sent no ranking", () => {
    // The key is missing: exactly today's actions, not picked by Scout.
    expect(rowChoices("need")).toEqual({
      own: [{ id: "reply", steps: ["reply"] }],
      universal: ["done", "snooze", "not_needed"],
      picked: false,
    });
    expect(rowChoices("toPay").own.map((c) => c.id)).toEqual(["remind", "paid"]);
  });

  it("offers the ranked actions in the server's order, at most three", () => {
    const choices = rowChoices("need", [
      { actions: ["reply"], likelihood: 91 },
      { actions: ["forward"], likelihood: 84 },
      { actions: ["remind"], likelihood: 80 },
      { actions: ["open_attachment"], likelihood: 75 },
    ]);
    expect(choices.picked).toBe(true);
    expect(choices.own.map((c) => c.id)).toEqual(["reply", "forward", "remind_when"]);
    // The universal actions stay as they are.
    expect(choices.universal).toEqual(["done", "snooze", "not_needed"]);
  });

  it("offers a pair done together as one button, run in order", () => {
    const choices = rowChoices("bill", [
      { actions: ["already_paid"], likelihood: 70 },
      { actions: ["still_to_pay"], likelihood: 30 },
      { actions: ["open_attachment", "forward"], likelihood: 81 },
    ]);
    expect(choices.own).toEqual([
      { id: "paid", steps: ["paid"] },
      { id: "to_pay", steps: ["to_pay"] },
      { id: "open_attachment+forward", steps: ["open_attachment", "forward"] },
    ]);
  });

  it("keeps a question's Yes and No, and never asks Still to pay twice", () => {
    const ranked = [
      { actions: ["already_paid" as const], likelihood: 60 },
      { actions: ["still_to_pay" as const], likelihood: 40 },
      { actions: ["remind" as const], likelihood: 90 },
    ];
    expect(rowChoices("question", ranked).own.map((c) => c.id)).toEqual(["yes", "no"]);
    expect(rowChoices("toPay", ranked).own.map((c) => c.id)).toEqual(["paid", "remind_when"]);
    // Nothing usable: the fixed actions.
    expect(rowChoices("need", []).picked).toBe(false);
  });

  it("offers a warning only its answers, and never a pair of two dialogs", () => {
    expect(
      rowChoices("warning", [
        { actions: ["safe"], likelihood: 80 },
        { actions: ["forward"], likelihood: 90 },
        { actions: ["report"], likelihood: 20 },
      ]).own.map((c) => c.id),
    ).toEqual(["safe", "report"]);
    // Reply with Forward would open two dialogs at once: skipped.
    expect(
      rowChoices("need", [
        { actions: ["reply", "forward"], likelihood: 85 },
        { actions: ["remind"], likelihood: 80 },
      ]).own.map((c) => c.id),
    ).toEqual(["remind_when"]);
  });

  it("never drops a bill's question or a warning's verdict from a ranking", () => {
    // The paid pair went unrated: the bill still asks it, first.
    expect(
      rowChoices("bill", [
        { actions: ["remind"], likelihood: 85 },
        { actions: ["forward"], likelihood: 80 },
      ]).own.map((c) => c.id),
    ).toEqual(["paid", "to_pay", "remind_when"]);
    expect(
      rowChoices("toPay", [{ actions: ["forward"], likelihood: 80 }]).own.map((c) => c.id),
    ).toEqual(["paid", "forward"]);
    expect(
      rowChoices("warning", [{ actions: ["safe"], likelihood: 80 }]).own.map((c) => c.id),
    ).toEqual(["report", "safe"]);
  });

  it("words Done today and says what can be undone", () => {
    expect(doneTodayKind(item("a", "reply", { snoozed_until: "x" }))).toEqual({
      key: "doneToday.snoozed",
      undo: "unsnooze",
    });
    const done = item("b", "pay", { state: "cleared", cleared_because: "done" });
    expect(doneTodayKind(done)).toEqual({ key: "doneToday.done", undo: "reopen" });
    const replied = item("c", "reply", { state: "cleared", cleared_because: "replied" });
    expect(doneTodayKind(replied)).toEqual({ key: "doneToday.replied", undo: undefined });
  });
});

describe("a reminder for a bill still to pay", () => {
  // Friday 16 October 2026, 15:00.
  const now = new Date(2026, 9, 16, 15);
  const at = (month: number, day: number) => new Date(2026, month - 1, day, 9).getTime();

  it("comes two days before the due date", () => {
    const remind = remindTime("2026-10-31", now);
    expect(remind.kind).toBe("before");
    expect(remind.until.getTime()).toBe(at(10, 29));
  });

  it("comes tomorrow at 09:00 when overdue or due too soon", () => {
    for (const due of ["2026-10-14", "2026-10-17"]) {
      const remind = remindTime(due, now);
      expect(remind.kind).toBe("tomorrow");
      expect(remind.until.getTime()).toBe(at(10, 17));
    }
  });

  it("comes on Monday when there is no due date", () => {
    const remind = remindTime(null, now);
    expect(remind.kind).toBe("monday");
    expect(remind.until.getTime()).toBe(at(10, 19));
  });
});

describe("ageLabel", () => {
  // Local times, so the test reads the same in every time zone.
  const now = new Date(2026, 9, 16, 15, 0);
  const at = (...parts: [number, number, number, number, number]) =>
    new Date(...parts).toISOString();

  it("says minutes, then hours, the same day", () => {
    expect(ageLabel(at(2026, 9, 16, 14, 48), now, "en-GB").text).toBe("12 minutes ago");
    expect(ageLabel(at(2026, 9, 16, 9, 30), now, "en-GB").text).toBe("5 hours ago");
  });

  it("names the day when asked for a date, whatever the age", () => {
    expect(ageLabel(at(2026, 9, 14, 10, 2), now, "en-GB", { asDate: true }).text).toBe("14 Oct");
    expect(ageLabel(at(2025, 11, 30, 10, 2), now, "en-GB", { asDate: true }).text).toBe(
      "30 Dec 2025",
    );
  });

  it("says now for what just arrived, even from a clock ahead", () => {
    expect(ageLabel(at(2026, 9, 16, 15, 1), now, "en-GB").text).toBe("Now");
  });

  it("counts calendar days in the week after", () => {
    expect(ageLabel(at(2026, 9, 15, 23, 50), now, "en-GB").text).toBe("Yesterday");
    expect(ageLabel(at(2026, 9, 12, 8, 0), now, "en-GB").text).toBe("4 days ago");
  });

  it("gives the date from a week on, with the year when it is another's", () => {
    expect(ageLabel(at(2026, 9, 2, 8, 0), now, "en-GB").text).toBe("2 Oct");
    expect(ageLabel(at(2025, 11, 20, 8, 0), now, "en-GB").text).toBe("20 Dec 2025");
  });

  it("speaks the member's language, capitalised at the start", () => {
    expect(ageLabel(at(2026, 9, 15, 10, 0), now, "pt-PT").text).toBe("Ontem");
    expect(ageLabel(at(2026, 9, 15, 10, 0), now, "fr").text).toBe("Hier");
  });

  it("keeps the instant and the full date for the tooltip", () => {
    const since = at(2026, 9, 12, 8, 0);
    const age = ageLabel(since, now, "en-GB");
    expect(age.dateTime).toBe(since);
    expect(age.label).toMatch(/Monday,? 12 October 2026/u);
  });
});
