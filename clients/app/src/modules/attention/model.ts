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
export type OwnActionId = "reply" | "yes" | "no" | "report" | "safe" | "paid" | "to_pay" | "remind";
export type UniversalActionId = "done" | "snooze" | "not_needed";

/**
 * What a row is: a question, a warning, a bill Campbooks asks about ("Is it
 * paid?"), a bill the member said is still to pay, or any other need.
 */
export type RowKind = "question" | "warning" | "need" | "bill" | "toPay";

/**
 * The actions Home offers for a need today. A question (`check`) is
 * answered Yes or No (No is "not needed", so Not needed is not repeated,
 * and Done means nothing until it is answered); a warning is cleared with
 * "It's safe" (its not needed) or reported and deleted (into spam, with a
 * smart rule offered after, campbooks-cloud#169); anything else is replied
 * to. #166 will
 * replace the own actions with a ranked list from the server.
 */
export function rowActionIds(kind: RowKind): {
  own: OwnActionId[];
  universal: UniversalActionId[];
} {
  switch (kind) {
    // Campbooks can't see the bank: Already paid or Still to pay. Not
    // needed is for mail that is no bill at all.
    case "bill": {
      return { own: ["paid", "to_pay"], universal: ["snooze", "not_needed"] };
    }
    // Still to pay: a reminder first, then Mark paid (its Done).
    case "toPay": {
      return { own: ["remind", "paid"], universal: ["snooze", "not_needed"] };
    }
    case "question": {
      return { own: ["yes", "no"], universal: ["snooze"] };
    }
    case "warning": {
      return { own: ["report", "safe"], universal: ["done", "snooze"] };
    }
    case "need": {
      return { own: ["reply"], universal: ["done", "snooze", "not_needed"] };
    }
  }
}

/**
 * When a bill still to pay should come back, from its due date only (never
 * from the mail's words): two days before the due date at 09:00; tomorrow at
 * 09:00 once that has passed or the bill is overdue; next Monday at 09:00
 * when there is no due date.
 */
export function remindTime(
  dueOn: string | null | undefined,
  now: Date,
): { until: Date; kind: "before" | "tomorrow" | "monday" } {
  const at = (date: Date, days: number) => {
    const out = new Date(date);
    out.setDate(out.getDate() + days);
    out.setHours(9, 0, 0, 0);
    return out;
  };
  if (dueOn === null || dueOn === undefined) {
    const toMonday = (8 - now.getDay()) % 7 || 7;
    return { until: at(now, toMonday), kind: "monday" };
  }
  const [year, month, day] = dueOn.split("-").map(Number);
  const due = new Date(year ?? 0, (month ?? 1) - 1, day ?? 1);
  const before = at(due, -2);
  if (before.getTime() > now.getTime()) {
    return { until: before, kind: "before" };
  }
  return { until: at(now, 1), kind: "tomorrow" };
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
