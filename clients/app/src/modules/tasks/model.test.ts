import { describe, expect, it } from "vitest";

import type { TaskView } from "./api/queries";
import {
  dayLabel,
  daysOverdue,
  dueChoices,
  dueThisWeek,
  groupOf,
  groupTasks,
  isoDay,
  laterChoices,
  laterOnDay,
} from "./model";

// Friday 23 October 2026, 10:30 local.
const NOW = new Date(2026, 9, 23, 10, 30);

function task(overrides: Partial<TaskView> = {}): TaskView {
  return {
    id: "t1",
    title: "Call the accountant",
    verb: "do",
    origin: "own",
    due_on: null,
    due_at: null,
    overdue: false,
    plan: "none",
    plan_on: null,
    later_until: null,
    state: "open",
    closed_at: null,
    assignee: "u1",
    created_by: "u1",
    handed_by: null,
    source: null,
    asks_done: false,
    revision: 1,
    created_at: "2026-10-20T09:00:00Z",
    updated_at: "2026-10-20T09:00:00Z",
    ...overrides,
  };
}

describe("isoDay", () => {
  it("writes the member's own calendar day", () => {
    expect(isoDay(NOW)).toBe("2026-10-23");
    expect(isoDay(new Date(2026, 0, 5, 23, 59))).toBe("2026-01-05");
  });
});

describe("groupOf", () => {
  it("puts a task past its due day under Overdue, and counts the days", () => {
    const late = task({ due_on: "2026-10-21" });

    expect(groupOf(late, NOW)).toBe("overdue");
    expect(daysOverdue(late, NOW)).toBe(2);
  });

  it("puts what is due or planned for today under Today, later days under Coming up", () => {
    expect(groupOf(task({ due_on: "2026-10-23" }), NOW)).toBe("today");
    expect(groupOf(task({ plan: "date", plan_on: "2026-10-23" }), NOW)).toBe("today");
    expect(groupOf(task({ due_on: "2026-10-30" }), NOW)).toBe("coming");
    // Due next week, planned for today: it is to be done today.
    expect(groupOf(task({ due_on: "2026-10-30", plan: "date", plan_on: "2026-10-23" }), NOW)).toBe(
      "today",
    );
  });

  it("puts an undated task under No date yet", () => {
    expect(groupOf(task(), NOW)).toBe("none");
  });

  it("puts a task under Later until its time, then back by its date", () => {
    const later = task({ plan: "later", later_until: new Date(2026, 9, 26, 9).toISOString() });

    expect(groupOf(later, NOW)).toBe("later");
    expect(groupOf(later, new Date(2026, 9, 26, 9, 1))).toBe("none");
  });
});

describe("groupTasks", () => {
  it("keeps the server's order inside a group and lists every group", () => {
    const groups = groupTasks(
      [
        task({ id: "a", due_on: "2026-10-30" }),
        task({ id: "b", due_on: "2026-10-31" }),
        task({ id: "c" }),
      ],
      NOW,
    );

    expect(groups.get("coming")?.map((t) => t.id)).toEqual(["a", "b"]);
    expect(groups.get("none")?.map((t) => t.id)).toEqual(["c"]);
    expect(groups.get("overdue")).toEqual([]);
  });
});

describe("dueThisWeek", () => {
  it("counts tasks due within seven days, overdue ones too", () => {
    expect(
      dueThisWeek(
        [
          task({ due_on: "2026-10-21" }),
          task({ due_on: "2026-10-29" }),
          task({ due_on: "2026-10-30" }),
          task(),
        ],
        NOW,
      ),
    ).toBe(2);
  });
});

describe("dueChoices", () => {
  it("offers today, tomorrow and the next weekday after that, skipping the weekend", () => {
    // Friday: tomorrow is Saturday, the next weekday is Monday the 26th.
    expect(dueChoices(NOW)).toEqual([
      { id: "today", day: "2026-10-23" },
      { id: "tomorrow", day: "2026-10-24" },
      { id: "weekday", day: "2026-10-26" },
    ]);
    // Monday: the next weekday after tomorrow is Wednesday.
    expect(dueChoices(new Date(2026, 9, 26, 8))[2]).toEqual({ id: "weekday", day: "2026-10-28" });
  });
});

describe("laterChoices", () => {
  it("offers tonight before 18:00, tomorrow morning and next Monday", () => {
    const [tonight, tomorrow, nextWeek] = laterChoices(NOW);

    expect(tonight?.id).toBe("tonight");
    expect(isoDay(tonight?.until ?? NOW)).toBe("2026-10-23");
    expect(tomorrow?.until.getHours()).toBe(9);
    expect(isoDay(tomorrow?.until ?? NOW)).toBe("2026-10-24");
    expect(nextWeek?.id).toBe("nextWeek");
    expect(isoDay(nextWeek?.until ?? NOW)).toBe("2026-10-26");
  });

  it("drops tonight once it is evening, and next week when tomorrow is Monday", () => {
    const evening = laterChoices(new Date(2026, 9, 23, 19)).map((choice) => choice.id);
    const sunday = laterChoices(new Date(2026, 9, 25, 10)).map((choice) => choice.id);

    expect(evening).toEqual(["tomorrow", "nextWeek"]);
    expect(sunday).toEqual(["tonight", "tomorrow"]);
  });

  it("ends a picked day at 09:00", () => {
    const until = laterOnDay("2026-11-02");

    expect(isoDay(until)).toBe("2026-11-02");
    expect(until.getHours()).toBe(9);
  });
});

describe("dayLabel", () => {
  it("writes the weekday and the day in the member's language", () => {
    expect(dayLabel("2026-10-30", "en")).toBe("Fri 30");
    expect(dayLabel("2026-10-30", "fr")).toMatch(/^ven\.? 30$/u);
  });
});
