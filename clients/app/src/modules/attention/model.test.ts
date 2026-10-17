import { describe, expect, it } from "vitest";

import type { ItemView } from "./api/queries";
import {
  doneTodayKind,
  groupByReason,
  remindTime,
  rowActionIds,
  rowChoices,
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
  it("groups by reason in Home's order and keeps the order inside", () => {
    const groups = groupByReason([item("a", "pay"), item("b", "reply"), item("c", "pay")]);
    expect(groups.map((g) => g.reason)).toEqual(["reply", "pay"]);
    expect(groups[1]?.items.map((i) => i.id)).toEqual(["a", "c"]);
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
