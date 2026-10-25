import type { CalendarView, EventView } from "./api/types";
import type { TaskRow } from "./due";
import { eventKey } from "./labels";
import {
  addDays,
  canPush,
  dayOf,
  daysBetween,
  isShown,
  lastDay,
  minutesOf,
  type Ymd,
} from "./model";

/**
 * The member's day as Home shows it (campbooks-cloud#359): the event that is
 * next up, the rest of today, and what is coming in the week after. Pure: the
 * clock is the member's wall clock in their calendar's zone (`nowIn`), the
 * same one the Calendar draws its now line with.
 */

/** A moment on the member's wall clock. */
export interface WallClock {
  day: Ymd;
  minute: number;
}

/** How far ahead Next up looks, in minutes. */
export const NEXT_UP_AHEAD = 120;
/** How long an event stays Next up once it started, in minutes. */
export const NEXT_UP_AFTER = 10;
/** Coming up: the days after today it covers. */
export const COMING_DAYS = 7;
/** Coming up: the most lines it shows. */
export const COMING_MAX = 6;

/** Minutes from `clock` to when a timed event starts (negative once it started). */
export function minutesUntil(event: EventView, clock: WallClock): number {
  return (
    daysBetween(clock.day, dayOf(event.starts)) * 1440 + minutesOf(event.starts) - clock.minute
  );
}

/** What Home shows at all: it stands (not cancelled) and the member did not decline it. */
function onHome(event: EventView): boolean {
  return isShown(event) && event.response !== "declined";
}

/**
 * The member's next event: the earliest timed one (not held time, not
 * all-day) that starts within the next two hours, or started less than ten
 * minutes ago. None when there is no such event.
 */
export function nextUp(events: readonly EventView[], clock: WallClock): EventView | undefined {
  return events
    .filter((event) => onHome(event) && !event.all_day && event.kind !== "hold")
    .map((event) => ({ event, until: minutesUntil(event, clock) }))
    .filter(({ until }) => until <= NEXT_UP_AHEAD && until > -NEXT_UP_AFTER)
    .toSorted((a, b) => a.until - b.until || a.event.title.localeCompare(b.event.title))[0]?.event;
}

/**
 * The rest of today, in start order, all-day first: the timed events and
 * held time that start from now on, and each all-day event covering today.
 * `next` (the Next up event) is left out.
 */
export function laterToday(
  events: readonly EventView[],
  clock: WallClock,
  next?: EventView,
): EventView[] {
  const skip = next === undefined ? undefined : eventKey(next);
  return events
    .filter((event) => {
      if (!onHome(event) || eventKey(event) === skip) {
        return false;
      }
      if (event.all_day) {
        return dayOf(event.starts) <= clock.day && clock.day <= lastDay(event);
      }
      return dayOf(event.starts) === clock.day && minutesOf(event.starts) >= clock.minute;
    })
    .toSorted(
      (a, b) =>
        Number(b.all_day) - Number(a.all_day) ||
        (a.all_day ? 0 : minutesOf(a.starts)) - (b.all_day ? 0 : minutesOf(b.starts)) ||
        a.title.localeCompare(b.title),
    );
}

/** One line of Coming up: an event, or a task on its due day (the Due layer's). */
export type Coming =
  { type: "event"; day: Ymd; event: EventView } | { type: "task"; day: Ymd; row: TaskRow };

/** When on its day a line falls, for the order: all-day and untimed first. */
function minuteOf(line: Coming): number {
  if (line.type === "task") {
    return line.row.minute ?? -1;
  }
  return line.event.all_day ? -1 : minutesOf(line.event.starts);
}

const titleOf = (line: Coming) => (line.type === "event" ? line.event.title : line.row.task.title);

/**
 * Coming up: the events (any kind) and the tasks due from tomorrow to a
 * week after it, soonest first, at most `limit`. A task's Planned row is not
 * a date it is due on, so only Due rows count.
 */
export function comingUp(
  events: readonly EventView[],
  rows: readonly TaskRow[],
  clock: WallClock,
  limit = COMING_MAX,
): Coming[] {
  const from = addDays(clock.day, 1);
  const to = addDays(clock.day, COMING_DAYS);
  const inWeek = (day: Ymd) => from <= day && day <= to;
  const lines: Coming[] = [
    ...events
      .filter((event) => onHome(event) && inWeek(dayOf(event.starts)))
      .map((event): Coming => ({ type: "event", day: dayOf(event.starts), event })),
    ...rows
      .filter((row) => row.kind === "due" && inWeek(row.stands))
      .map((row): Coming => ({ type: "task", day: row.stands, row })),
  ];
  return lines
    .toSorted(
      (a, b) =>
        a.day.localeCompare(b.day) ||
        minuteOf(a) - minuteOf(b) ||
        titleOf(a).localeCompare(titleOf(b)),
    )
    .slice(0, limit);
}

/**
 * Whether a Coming up line offers "Add to my calendar": an event only the
 * member sees, that could go on a writable calendar of their provider.
 * Held time stays where Campbooks keeps it.
 */
export function offersAdd(event: EventView, calendars: readonly CalendarView[]): boolean {
  return event.kind !== "hold" && canPush(event, calendars);
}

/**
 * Where "Add to my calendar" puts an event without asking: the calendar the
 * member added to last, else their primary writable one, else the first.
 */
export function pushTarget(
  calendars: readonly CalendarView[],
  lastUsed: string | null | undefined,
): CalendarView | undefined {
  const writable = calendars.filter(
    (calendar) => calendar.source === "provider" && calendar.writable,
  );
  return (
    writable.find((calendar) => calendar.id === lastUsed) ??
    writable.find((calendar) => calendar.primary) ??
    writable[0]
  );
}

/** A `place` that is a web address the member can join at (`https://` only). */
export function joinUrl(event: EventView): URL | undefined {
  const place = event.place?.trim() ?? "";
  if (!place.startsWith("https://")) {
    return undefined;
  }
  try {
    const url = new URL(place);
    return url.protocol === "https:" ? url : undefined;
  } catch {
    return undefined;
  }
}

/**
 * The facts of the hero's line: the next timed event of today (not held
 * time) and how many more rows today holds after it. None without one.
 */
export function dayFacts(
  rows: readonly EventView[],
): { next: EventView; more: number } | undefined {
  const next = rows.find((event) => !event.all_day && event.kind !== "hold");
  return next === undefined ? undefined : { next, more: rows.length - 1 };
}
