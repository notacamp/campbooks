// Pure helpers of Home's "What needs you": grouping, Later's times and the
// sentences of Done today. No React, no fetching.
import type { ItemView, Reason } from "./api/queries";

/** The groups, in the order Home shows them. */
export const REASONS: readonly Reason[] = ["review", "reply", "decide", "do", "pay"];

/** Items by reason, in display order, keeping the server's order inside each group. */
export function groupByReason(items: readonly ItemView[]): { reason: Reason; items: ItemView[] }[] {
  return REASONS.flatMap((reason) => {
    const inGroup = items.filter((item) => item.reason === reason);
    return inGroup.length === 0 ? [] : [{ reason, items: inGroup }];
  });
}

/** The choices of "Later", from `now`: later today (18:00), tomorrow and next Monday (08:00). */
export function laterOptions(now: Date): { key: "today" | "tomorrow" | "monday"; until: Date }[] {
  const at = (days: number, hour: number) => {
    const date = new Date(now);
    date.setDate(date.getDate() + days);
    date.setHours(hour, 0, 0, 0);
    return date;
  };
  const toMonday = ((8 - now.getDay()) % 7) + (now.getDay() === 1 ? 7 : 0);
  const options: { key: "today" | "tomorrow" | "monday"; until: Date }[] = [];
  const evening = at(0, 18);
  if (evening.getTime() - now.getTime() > 60 * 60 * 1000) {
    options.push({ key: "today", until: evening });
  }
  options.push({ key: "tomorrow", until: at(1, 8) });
  options.push({ key: "monday", until: at(toMonday === 0 ? 7 : toMonday, 8) });
  return options;
}

/** Which sentence Done today uses for an item, and how it can be undone. */
export function doneTodayKind(item: ItemView): {
  key: string;
  undo: "reopen" | "unsnooze" | undefined;
} {
  if (item.state === "open") {
    return { key: "doneToday.later", undo: "unsnooze" };
  }
  switch (item.cleared_because) {
    case "done": {
      return { key: "doneToday.done", undo: "reopen" };
    }
    case "not_needed": {
      return { key: "doneToday.notNeeded", undo: "reopen" };
    }
    case "replied": {
      return { key: "doneToday.replied", undo: undefined };
    }
    case "archived": {
      return { key: "doneToday.archived", undo: undefined };
    }
    default: {
      return { key: "doneToday.gone", undo: undefined };
    }
  }
}
