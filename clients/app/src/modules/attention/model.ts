// Pure helpers of Home's "What needs you": grouping, Snooze's times, the
// actions a row offers and the
// sentences of Done today. No React, no fetching.
import type { ItemView, Reason } from "./api/queries";

/** The groups, in the order Home shows them. */
const REASONS: readonly Reason[] = ["review", "reply", "decide", "do", "pay"];

/** Items by reason, in display order, keeping the server's order inside each group. */
export function groupByReason(items: readonly ItemView[]): { reason: Reason; items: ItemView[] }[] {
  return REASONS.flatMap((reason) => {
    const inGroup = items.filter((item) => item.reason === reason);
    return inGroup.length === 0 ? [] : [{ reason, items: inGroup }];
  });
}

export type SnoozeKey = "today" | "tomorrow" | "monday" | "nextWeek";

/**
 * The choices of Snooze, from `now`: later today (18:00, while that is more
 * than an hour away), tomorrow, next Monday and a week from today (08:00).
 * A day that would repeat an earlier choice is left out (Monday on a
 * Sunday, next week on a Monday).
 */
export function snoozeOptions(now: Date): { key: SnoozeKey; until: Date }[] {
  const at = (days: number, hour: number) => {
    const date = new Date(now);
    date.setDate(date.getDate() + days);
    date.setHours(hour, 0, 0, 0);
    return date;
  };
  const toMonday = (8 - now.getDay()) % 7 || 7;
  const evening = at(0, 18);
  const worthIt = evening.getTime() - now.getTime() > 60 * 60 * 1000;
  const options: { key: SnoozeKey; until: Date }[] = [
    ...(worthIt ? [{ key: "today" as const, until: evening }] : []),
    { key: "tomorrow", until: at(1, 8) },
    { key: "monday", until: at(toMonday, 8) },
    { key: "nextWeek", until: at(7, 8) },
  ];
  return options.filter(
    (option, index) =>
      !options
        .slice(0, index)
        .some((earlier) => earlier.until.getTime() === option.until.getTime()),
  );
}

/** What a row offers: the item's own actions (keys 1 to 3), then the universal ones. */
export type OwnActionId = "reply" | "yes" | "no" | "safe";
export type UniversalActionId = "done" | "snooze" | "not_needed";

/**
 * The actions Home offers for a need today. A question (`check`) is
 * answered Yes or No (No is "not needed", so Not needed is not repeated,
 * and Done means nothing until it is answered); a warning is cleared with
 * "It's safe" (its not needed); anything else is replied to. #166 will
 * replace the own actions with a ranked list from the server.
 */
export function rowActionIds(kind: "question" | "warning" | "need"): {
  own: OwnActionId[];
  universal: UniversalActionId[];
} {
  switch (kind) {
    case "question": {
      return { own: ["yes", "no"], universal: ["snooze"] };
    }
    case "warning": {
      return { own: ["safe"], universal: ["done", "snooze"] };
    }
    case "need": {
      return { own: ["reply"], universal: ["done", "snooze", "not_needed"] };
    }
  }
}

/** Which sentence Done today uses for an item, and how it can be undone. */
export function doneTodayKind(item: ItemView): {
  key: string;
  undo: "reopen" | "unsnooze" | undefined;
} {
  if (item.state === "open") {
    return { key: "doneToday.snoozed", undo: "unsnooze" };
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
