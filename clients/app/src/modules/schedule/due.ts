import { useTasks, type TaskView } from "~/modules/tasks";

import { daysBetween, nowIn, type Ymd } from "./model";

/**
 * The tasks on the calendar. They are the member's own open tasks (the tasks
 * module's list, live through the change feed), drawn on the day each is due
 * (a Due row) and on the day the member planned it for (a Planned row).
 */
export interface TaskRow {
  /** `due:<task id>` or `plan:<task id>`: what the address holds for its sheet. */
  key: string;
  kind: "due" | "planned";
  task: TaskView;
  /** The day it stands on: its own day, or today once that day has passed. */
  stands: Ymd;
  /** Whole days its own day is behind today (0 when it is not). */
  late: number;
  /** A Due row with a time: minutes after midnight on the member's clock. */
  minute: number | undefined;
  /** A Due row whose task is also planned for that same day. */
  alsoPlanned: boolean;
}

const dueKeyOf = (taskId: string) => `due:${taskId}`;
const planKeyOf = (taskId: string) => `plan:${taskId}`;

const later = (day: Ymd, today: Ymd): Ymd => (day < today ? today : day);
const behind = (day: Ymd, today: Ymd): number => Math.max(daysBetween(day, today), 0);

/** The minutes after midnight a task is due at on the member's clock, when it has a time. */
function dueMinute(task: TaskView, zone: string): number | undefined {
  if (task.due_at === null || task.due_at === undefined) {
    return undefined;
  }
  return nowIn(zone, new Date(task.due_at)).minute;
}

/**
 * The rows of a list of open tasks as of `today`. A task has at most one row
 * on a day: when its plan and its due date stand on the same day, the Due row
 * shows alone. A Later plan has no Planned row.
 */
export function taskRows(tasks: readonly TaskView[], today: Ymd, zone: string): TaskRow[] {
  return tasks.flatMap((task): TaskRow[] => {
    if (task.state !== "open") {
      return [];
    }
    const due = task.due_on ?? undefined;
    const plan = task.plan === "date" ? (task.plan_on ?? undefined) : undefined;
    const dueStands = due === undefined ? undefined : later(due, today);
    const planStands = plan === undefined ? undefined : later(plan, today);
    const rows: TaskRow[] = [];
    if (due !== undefined && dueStands !== undefined) {
      rows.push({
        key: dueKeyOf(task.id),
        kind: "due",
        task,
        stands: dueStands,
        late: behind(due, today),
        minute: dueMinute(task, zone),
        alsoPlanned: planStands === dueStands,
      });
    }
    if (plan !== undefined && planStands !== undefined && planStands !== dueStands) {
      rows.push({
        key: planKeyOf(task.id),
        kind: "planned",
        task,
        stands: planStands,
        late: behind(plan, today),
        minute: undefined,
        alsoPlanned: false,
      });
    }
    return rows;
  });
}

/** The member's open tasks as calendar rows. Empty while they load. */
export function useTaskRows(workspaceId: string, today: Ymd, zone: string): readonly TaskRow[] {
  const tasks = useTasks(workspaceId);
  return taskRows(tasks.data ?? [], today, zone);
}
