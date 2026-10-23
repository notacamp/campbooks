import type { TaskView } from "./api/queries";
import { dayLabel, daysBetween, daysOverdue, isLater, isoDay, whenLabel } from "./model";

/** `t` for keys only known at runtime. */
export type TaskTranslate = (key: string, options?: Record<string, unknown>) => string;

/**
 * When it is due, in words: "Overdue · 2 days" (late), "Due today", "Due
 * tomorrow" or "Due Fri 30"; nothing for an undated task.
 */
export function dueText(
  task: TaskView,
  t: TaskTranslate,
  locale: string,
  now: Date,
): { text: string; late: boolean } | undefined {
  if (task.due_on === null || task.due_on === undefined) {
    return undefined;
  }
  const late = daysOverdue(task, now);
  if (late > 0) {
    return { text: t("due.overdue", { count: late }), late: true };
  }
  const days = daysBetween(isoDay(now), task.due_on);
  if (days === 0) {
    return { text: t("due.today"), late: false };
  }
  return days === 1
    ? { text: t("due.tomorrow"), late: false }
    : { text: t("due.on", { date: dayLabel(task.due_on, locale) }), late: false };
}

/** Its plan, in words: "Planned Wed 28", "Later · Mon 2 Nov"; nothing for no plan. */
export function planText(
  task: TaskView,
  t: TaskTranslate,
  locale: string,
  now: Date,
): string | undefined {
  if (task.plan === "date" && task.plan_on !== null && task.plan_on !== undefined) {
    return t("plan.planned", { date: dayLabel(task.plan_on, locale) });
  }
  if (isLater(task, now) && task.later_until !== null && task.later_until !== undefined) {
    return t("plan.later", { date: whenLabel(task.later_until, locale) });
  }
  return undefined;
}
