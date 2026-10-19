// Pure helpers of Home's "What needs you": grouping, Snooze's times, the
// actions a row offers and the
// sentences of Done today. No React, no fetching.
import type { Schema } from "~/lib/api";

import type { ItemView, Reason } from "./api/queries";

/** The groups, in the order Home shows them. */
const REASONS: readonly Reason[] = ["review", "reply", "decide", "do", "pay", "worth_a_look"];

/**
 * The signal colour of a group: its reason's, except "Worth a look", which
 * has a hue of its own (`look`).
 */
export function signalOf(reason: Reason): Exclude<Reason, "worth_a_look"> | "look" {
  return reason === "worth_a_look" ? "look" : reason;
}

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

/**
 * What a row offers: the item's own actions (keys 1 to 3), then the universal
 * ones. `remind` is a bill's one-press reminder from its due date;
 * `remind_when`, `open_attachment` and `forward` come from the ranked
 * catalogue (campbooks-cloud#166): Remind asks when, Open attachment shows
 * the file in the opened item, Forward asks to whom.
 */
export type OwnActionId =
  | "reply"
  | "yes"
  | "no"
  | "report"
  | "safe"
  | "paid"
  | "to_pay"
  | "remind"
  | "remind_when"
  | "open_attachment"
  | "forward";
export type UniversalActionId = "done" | "snooze" | "not_needed";

/**
 * What a row is: a question, a warning, a bill Campbooks asks about ("Is it
 * paid?"), a bill the member said is still to pay, mail a rule they taught
 * keeps in front of them until opened ("Worth a look"), or any other need.
 */
export type RowKind = "question" | "warning" | "need" | "bill" | "toPay" | "highlight";

/**
 * The actions Home offers for a need today. A question (`check`) is
 * answered Yes or No (No is "not needed", so Not needed is not repeated,
 * and Done means nothing until it is answered); a warning is cleared with
 * "It's safe" (its not needed) or reported and deleted (into spam, with a
 * smart rule offered after, campbooks-cloud#169); anything else is replied
 * to. A need the server ranked offers its own actions instead
 * ({@link rowChoices}, campbooks-cloud#166).
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
    // Nothing is asked of the member: it clears itself once opened. Seen
    // (Done) and Later are the only buttons; there is nothing to say "not
    // needed" about, the rule that raised it is theirs.
    case "highlight": {
      return { own: [], universal: ["done", "snooze"] };
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

/** One action of the catalogue, as the server names it (campbooks-cloud#166). */
type CatalogueAction = Schema<"NeedAction">;
type SuggestedAction = Schema<"SuggestedAction">;

/** A catalogue action as the row runs it. */
const FROM_CATALOGUE: Record<CatalogueAction, OwnActionId> = {
  reply: "reply",
  remind: "remind_when",
  open_attachment: "open_attachment",
  forward: "forward",
  report: "report",
  safe: "safe",
  already_paid: "paid",
  still_to_pay: "to_pay",
};

/**
 * One button of the row's own actions: one action, or a pair usually done
 * together, offered as one and run in order.
 */
export interface OwnChoice {
  /** Stable id: the action's, or both joined with `+`. */
  id: string;
  steps: OwnActionId[];
}

/** At most three own actions, on keys 1 to 3. */
const MAX_OWN = 3;

/** Own actions that ask a follow-up in a dialog: a pair holds at most one. */
const FOLLOW_UPS: ReadonlySet<OwnActionId> = new Set(["reply", "remind_when", "forward"]);

/** A warning's answers: whatever the server ranks, a warning offers only these. */
const WARNING_ANSWERS: ReadonlySet<OwnActionId> = new Set(["report", "safe"]);

/**
 * The answers a row can't do without, ranked or not: a bill's question, a
 * bill still to pay's Mark paid, a warning's verdict. Kept first, in their
 * fixed order, when the ranking left one out (its rating was missing).
 */
const REQUIRED: Partial<Record<RowKind, readonly OwnActionId[]>> = {
  bill: ["paid", "to_pay"],
  toPay: ["paid"],
  warning: ["report", "safe"],
};

/**
 * What a row offers. With the server's ranked `actions` (the catalogue is on
 * and the message was rated), its own actions are those, in the server's
 * order, `picked` by Scout; a pair is one button. Without them (the key is
 * missing), exactly the fixed actions of {@link rowActionIds}. A question
 * keeps Yes and No (its answer is the point), a bill already said to be
 * still to pay never offers Still to pay again, a warning offers only its
 * answers, a pair never holds two follow-up dialogs, and a row's required
 * answers ({@link REQUIRED}) are always there. Nothing usable left in the ranking
 * falls back to the fixed actions too.
 */
export function rowChoices(
  kind: RowKind,
  ranked?: readonly SuggestedAction[] | null,
): { own: OwnChoice[]; universal: UniversalActionId[]; picked: boolean } {
  const fixed = rowActionIds(kind);
  const fallback = {
    own: fixed.own.map((id) => ({ id, steps: [id] })),
    universal: fixed.universal,
    picked: false,
  };
  // Only a missing key means "not rated": `null` never comes from the server.
  if (ranked === undefined || kind === "question" || kind === "highlight") {
    return fallback;
  }
  const own: OwnChoice[] = [];
  for (const suggestion of ranked ?? []) {
    const steps = suggestion.actions.map((action) => FROM_CATALOGUE[action]);
    const skipped =
      steps.length === 0 ||
      (kind === "toPay" && steps.includes("to_pay")) ||
      (kind === "warning" && !steps.every((step) => WARNING_ANSWERS.has(step))) ||
      steps.filter((step) => FOLLOW_UPS.has(step)).length > 1;
    if (skipped) {
      continue;
    }
    const id = steps.join("+");
    if (!own.some((choice) => choice.id === id)) {
      own.push({ id, steps });
    }
  }
  if (own.length === 0) {
    return fallback;
  }
  const missing = (REQUIRED[kind] ?? []).filter(
    (id) => !own.some((choice) => choice.steps.length === 1 && choice.steps[0] === id),
  );
  const all = [...missing.map((id) => ({ id, steps: [id] })), ...own];
  return { own: all.slice(0, MAX_OWN), universal: fixed.universal, picked: true };
}

const HOUR_MS = 60 * 60 * 1000;

/**
 * How long an item has waited, from `since` (when its cause arrived): "5
 * minutes ago" and "3 hours ago" the same day, "Yesterday" and "4 days ago"
 * in the week after, the date from a week on ("12 Oct", with the year when it
 * is another year's). `label` is the full date and time, for the tooltip.
 */
export function ageLabel(
  since: string,
  now: Date,
  locale: string,
): { text: string; dateTime: string; label: string } {
  const at = new Date(since);
  const label = new Intl.DateTimeFormat(locale, { dateStyle: "full", timeStyle: "short" }).format(
    at,
  );
  const relative = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  // A clock a little ahead of the server's: it just arrived.
  const ms = Math.max(0, now.getTime() - at.getTime());
  const midnight = (date: Date) =>
    new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((midnight(now) - midnight(at)) / (24 * HOUR_MS));
  let text: string;
  if (ms < 60_000) {
    // "Now", not "this minute".
    text = relative.format(0, "second");
  } else if (days <= 0 && ms < HOUR_MS) {
    text = relative.format(-Math.floor(ms / 60_000), "minute");
  } else if (days <= 0) {
    text = relative.format(-Math.floor(ms / HOUR_MS), "hour");
  } else if (days < 7) {
    text = relative.format(-days, "day");
  } else {
    text = new Intl.DateTimeFormat(locale, {
      day: "numeric",
      month: "short",
      ...(at.getFullYear() === now.getFullYear() ? {} : { year: "numeric" }),
    }).format(at);
  }
  // "yesterday" starts the line: capitalised as the locale does.
  return {
    text: text.charAt(0).toLocaleUpperCase(locale) + text.slice(1),
    dateTime: at.toISOString(),
    label,
  };
}
