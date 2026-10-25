import { describe, expect, it } from "vitest";

import { LEASE } from "../../../test/demo/tasks";
import type { TaskView } from "~/modules/tasks";

import { taskRows } from "./due";

const TODAY = "2026-10-27";
const ZONE = "Europe/Lisbon";

const task = (over: Partial<TaskView>): TaskView => ({
  ...LEASE,
  due_on: null,
  due_at: null,
  plan: "none",
  plan_on: null,
  ...over,
});

const rows = (...tasks: TaskView[]) =>
  taskRows(tasks, TODAY, ZONE).map((row) => [row.kind, row.stands, row.late] as const);

describe("the calendar's task rows", () => {
  it("shows a task on its due day and on the day it is planned, when they differ", () => {
    expect(rows(task({ due_on: "2026-10-30", plan: "date", plan_on: "2026-10-28" }))).toEqual([
      ["due", "2026-10-30", 0],
      ["planned", "2026-10-28", 0],
    ]);
  });

  it("shows the Due row alone, saying it is planned, when both stand on one day", () => {
    const [due, ...rest] = taskRows(
      [task({ due_on: "2026-10-29", plan: "date", plan_on: "2026-10-29" })],
      TODAY,
      ZONE,
    );
    expect(rest).toEqual([]);
    expect(due?.kind).toBe("due");
    expect(due?.alsoPlanned).toBe(true);
  });

  it("stands an overdue task on today", () => {
    expect(rows(task({ due_on: "2026-10-25" }))).toEqual([["due", TODAY, 2]]);
  });

  it("stands a slipped plan on today, as Planned", () => {
    expect(rows(task({ plan: "date", plan_on: "2026-10-24" }))).toEqual([["planned", TODAY, 3]]);
  });

  it("shows one Due row when a slipped plan and an overdue date both land on today", () => {
    const [due, ...rest] = taskRows(
      [task({ due_on: "2026-10-25", plan: "date", plan_on: "2026-10-24" })],
      TODAY,
      ZONE,
    );
    expect(rest).toEqual([]);
    expect(due?.alsoPlanned).toBe(true);
  });

  it("gives a Later task no Planned row, and keeps its Due row", () => {
    expect(
      rows(task({ due_on: "2026-10-31", plan: "later", later_until: "2026-11-02T09:00:00Z" })),
    ).toEqual([["due", "2026-10-31", 0]]);
  });

  it("shows nothing for a task with no day, or one that is not open", () => {
    expect(rows(task({}))).toEqual([]);
    expect(rows(task({ due_on: "2026-10-30", state: "done" }))).toEqual([]);
  });

  it("reads the time a task is due at on the member's clock", () => {
    const [row] = taskRows(
      [task({ due_on: "2026-10-30", due_at: "2026-10-30T17:00:00Z" })],
      TODAY,
      ZONE,
    );
    expect(row?.minute).toBe(17 * 60);
    const [dated] = taskRows([task({ due_on: "2026-10-30" })], TODAY, ZONE);
    expect(dated?.minute).toBeUndefined();
  });

  it("keys the two rows of a task apart", () => {
    const keys = taskRows(
      [task({ due_on: "2026-10-30", plan: "date", plan_on: "2026-10-28" })],
      TODAY,
      ZONE,
    ).map((row) => row.key);
    expect(new Set(keys).size).toBe(2);
  });

  it("draws no Planned row for a held task, and no Due row on its held due day", () => {
    const hold = (starts: string) => ({
      event_id: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9e10",
      calendar_id: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9e11",
      starts,
      ends: starts.replace("T09:00", "T09:30"),
    });
    // Held Wednesday 09:00 in Lisbon, due Thursday: the Due row only.
    expect(
      rows(
        task({
          due_on: "2026-10-29",
          plan: "date",
          plan_on: "2026-10-28",
          hold: hold("2026-10-28T09:00:00Z"),
        }),
      ),
    ).toEqual([["due", "2026-10-29", 0]]);
    // Held on its due day: the block stands for both.
    expect(
      rows(
        task({
          due_on: "2026-10-29",
          plan: "date",
          plan_on: "2026-10-29",
          hold: hold("2026-10-29T09:00:00Z"),
        }),
      ),
    ).toEqual([]);
  });
});
