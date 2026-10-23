import type { TaskView } from "./api/queries";

/** The groups of the Tasks list, in the order they are shown. */
export type Group = "overdue" | "today" | "coming" | "none" | "later";

export const GROUPS: readonly Group[] = ["overdue", "today", "coming", "none", "later"];

const DAY_MS = 86_400_000;

const two = (n: number) => String(n).padStart(2, "0");

/** A calendar day ("2026-10-23") in the member's own time zone. */
export function isoDay(date: Date): string {
  return `${String(date.getFullYear())}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
}

/** `date` plus `days` calendar days, at the same local time. */
export function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

/** A day written "2026-10-23", at UTC midnight. */
export function utcDay(iso: string): Date {
  const [year, month, day] = iso.split("-").map(Number);
  return new Date(Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1));
}

/** Whole days from `from` to `to` (both "2026-10-23"): positive when `to` is later. */
export function daysBetween(from: string, to: string): number {
  return Math.round((utcDay(to).getTime() - utcDay(from).getTime()) / DAY_MS);
}

/** Whether the task is planned Later and that time has not come yet. */
export function isLater(task: TaskView, now: Date): boolean {
  return (
    task.plan === "later" &&
    task.later_until !== null &&
    task.later_until !== undefined &&
    new Date(task.later_until).getTime() > now.getTime()
  );
}

/** The day the member means to do it: the planned day, else the due day. */
export function doingDay(task: TaskView): string | undefined {
  if (task.plan === "date" && task.plan_on !== null && task.plan_on !== undefined) {
    return task.plan_on;
  }
  return task.due_on ?? undefined;
}

/** How many days past its due day it is (0 when it is not). */
export function daysOverdue(task: TaskView, now: Date): number {
  if (task.due_on === null || task.due_on === undefined) {
    return 0;
  }
  return Math.max(0, daysBetween(task.due_on, isoDay(now)));
}

/** Which group an open task is in. */
export function groupOf(task: TaskView, now: Date): Group {
  if (isLater(task, now)) {
    return "later";
  }
  if (daysOverdue(task, now) > 0) {
    return "overdue";
  }
  const day = doingDay(task);
  if (day === undefined) {
    return "none";
  }
  return day <= isoDay(now) ? "today" : "coming";
}

/** Open tasks by group, each keeping the server's order (Later by when it ends). */
export function groupTasks(tasks: readonly TaskView[], now: Date): Map<Group, TaskView[]> {
  const groups = new Map<Group, TaskView[]>(GROUPS.map((group) => [group, []]));
  for (const task of tasks) {
    groups.get(groupOf(task, now))?.push(task);
  }
  groups
    .get("later")
    ?.sort((a, b) => (a.later_until ?? "").localeCompare(b.later_until ?? ""));
  return groups;
}

/** The tasks in the order the list shows them, group by group (Later only when open). */
export function listOrder(
  groups: ReadonlyMap<Group, readonly TaskView[]>,
  laterOpen: boolean,
): TaskView[] {
  return GROUPS.flatMap((group) =>
    group === "later" && !laterOpen ? [] : (groups.get(group) ?? []),
  );
}

/** How many open tasks are due in the next seven days (today included). */
export function dueThisWeek(tasks: readonly TaskView[], now: Date): number {
  const today = isoDay(now);
  return tasks.filter((task) => {
    if (task.due_on === null || task.due_on === undefined) {
      return false;
    }
    const days = daysBetween(today, task.due_on);
    return days <= 6;
  }).length;
}

/** The due chips of the quick add: Today, Tomorrow and the next weekday after that. */
export interface DueChoice {
  id: "today" | "tomorrow" | "weekday";
  /** "2026-10-26". */
  day: string;
}

export function dueChoices(now: Date): DueChoice[] {
  const tomorrow = addDays(now, 1);
  let weekday = addDays(now, 2);
  // Saturday (6) and Sunday (0) are not working days.
  while (weekday.getDay() === 0 || weekday.getDay() === 6) {
    weekday = addDays(weekday, 1);
  }
  return [
    { id: "today", day: isoDay(now) },
    { id: "tomorrow", day: isoDay(tomorrow) },
    { id: "weekday", day: isoDay(weekday) },
  ];
}

/** The times Later offers. */
export interface LaterChoice {
  id: "tonight" | "tomorrow" | "nextWeek";
  until: Date;
}

const at = (date: Date, hours: number) => {
  const next = new Date(date);
  next.setHours(hours, 0, 0, 0);
  return next;
};

/** Tonight (19:00, while it is before 18:00), tomorrow at 09:00, next Monday at 09:00. */
export function laterChoices(now: Date): LaterChoice[] {
  const choices: LaterChoice[] = [];
  if (now.getHours() < 18) {
    choices.push({ id: "tonight", until: at(now, 19) });
  }
  choices.push({ id: "tomorrow", until: at(addDays(now, 1), 9) });
  // The next Monday, never today.
  const toMonday = (8 - now.getDay()) % 7 || 7;
  // On a Sunday that is tomorrow, which is already offered.
  if (toMonday > 1) {
    choices.push({ id: "nextWeek", until: at(addDays(now, toMonday), 9) });
  }
  return choices;
}

/** A picked day as a time Later can end at: 09:00 that day. */
export function laterOnDay(day: string): Date {
  const [year, month, date] = day.split("-").map(Number);
  return new Date(year ?? 0, (month ?? 1) - 1, date ?? 1, 9, 0, 0, 0);
}

/** A day ("2026-10-30") as "Fri 30" in `locale`. */
export function dayLabel(day: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { timeZone: "UTC", weekday: "short", day: "numeric" }).format(
    utcDay(day),
  );
}

/** A time as "Mon 2 Nov" in `locale`. */
export function whenLabel(when: string | Date, locale: string): string {
  return new Intl.DateTimeFormat(locale, { weekday: "short", day: "numeric", month: "short" }).format(
    new Date(when),
  );
}
