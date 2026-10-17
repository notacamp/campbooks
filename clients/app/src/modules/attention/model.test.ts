import { describe, expect, it } from "vitest";

import type { ItemView } from "./api/queries";
import { doneTodayKind, groupByReason, remindTime, rowActionIds, snoozeOptions } from "./model";

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
    expect(rowActionIds("warning")).toEqual({ own: ["safe"], universal: ["done", "snooze"] });
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
