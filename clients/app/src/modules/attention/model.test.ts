import { describe, expect, it } from "vitest";

import type { ItemView } from "./api/queries";
import { doneTodayKind, groupByReason, laterOptions } from "./model";

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

  it("offers later today only while it is worth it", () => {
    // Tuesday 13 October 2026, 09:00 local.
    const morning = new Date(2026, 9, 13, 9, 0);
    expect(laterOptions(morning).map((o) => o.key)).toEqual(["today", "tomorrow", "monday"]);
    const monday = laterOptions(morning).at(-1)?.until;
    expect(monday?.getDay()).toBe(1);
    expect(monday?.getDate()).toBe(19);
    const evening = new Date(2026, 9, 13, 17, 30);
    expect(laterOptions(evening).map((o) => o.key)).toEqual(["tomorrow", "monday"]);
    // On a Monday, Monday means next week.
    expect(
      laterOptions(new Date(2026, 9, 19, 9, 0))
        .at(-1)
        ?.until.getDate(),
    ).toBe(26);
  });

  it("words Done today and says what can be undone", () => {
    expect(doneTodayKind(item("a", "reply", { snoozed_until: "x" }))).toEqual({
      key: "doneToday.later",
      undo: "unsnooze",
    });
    const done = item("b", "pay", { state: "cleared", cleared_because: "done" });
    expect(doneTodayKind(done)).toEqual({ key: "doneToday.done", undo: "reopen" });
    const replied = item("c", "reply", { state: "cleared", cleared_because: "replied" });
    expect(doneTodayKind(replied)).toEqual({ key: "doneToday.replied", undo: undefined });
  });
});
