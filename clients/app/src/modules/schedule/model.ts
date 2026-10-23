import type { EventView } from "./api/types";

/**
 * Dates and times of the calendar. The server writes every time with the
 * offset of the member's own time zone (`2026-11-06T07:40:00+00:00`) and an
 * all-day event as a plain date, so the member's wall clock is read straight
 * out of the text: no zone arithmetic, no `Date` in the local zone.
 */

/** A calendar day, `YYYY-MM-DD`. */
export type Ymd = string;

export const VIEWS = ["agenda", "week", "month"] as const;
export type View = (typeof VIEWS)[number];

export function isView(value: unknown): value is View {
  return typeof value === "string" && (VIEWS as readonly string[]).includes(value);
}

const DAY_MS = 86_400_000;

/** A day as a UTC date, so adding days never meets a daylight-saving change. */
function utc(day: Ymd): Date {
  const [year = 1970, month = 1, date = 1] = day.split("-").map(Number);
  return new Date(Date.UTC(year, month - 1, date));
}

function ymd(date: Date): Ymd {
  return date.toISOString().slice(0, 10);
}

export function isYmd(value: unknown): value is Ymd {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/u.test(value)) {
    return false;
  }
  const date = utc(value);
  return !Number.isNaN(date.getTime()) && ymd(date) === value;
}

export function addDays(day: Ymd, count: number): Ymd {
  return ymd(new Date(utc(day).getTime() + count * DAY_MS));
}

/** Whole days from `from` to `to` (negative when `to` is earlier). */
export function daysBetween(from: Ymd, to: Ymd): number {
  return Math.round((utc(to).getTime() - utc(from).getTime()) / DAY_MS);
}

/** The Monday of `day`'s week. */
export function weekStart(day: Ymd): Ymd {
  const weekday = (utc(day).getUTCDay() + 6) % 7;
  return addDays(day, -weekday);
}

export function monthStart(day: Ymd): Ymd {
  return `${day.slice(0, 7)}-01`;
}

/** The first day of the month `count` months from `day`'s. */
export function addMonths(day: Ymd, count: number): Ymd {
  const date = utc(day);
  return ymd(new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + count, 1)));
}

/** Whether two event times are the same moment (or the same day, for dates). */
export function sameMoment(a: string, b: string): boolean {
  return a.length === 10 || b.length === 10
    ? a.slice(0, 10) === b.slice(0, 10)
    : Date.parse(a) === Date.parse(b);
}

/** The day part of an event time (`starts`, `ends`). */
export function dayOf(time: string): Ymd {
  return time.slice(0, 10);
}

/** Minutes after midnight on the wall clock of an event time; 0 for a plain date. */
export function minutesOf(time: string): number {
  if (time.length < 16) {
    return 0;
  }
  return Number(time.slice(11, 13)) * 60 + Number(time.slice(14, 16));
}

/** What the calendar shows at once, from `from` up to (not including) `to`. */
export interface Period {
  from: Ymd;
  to: Ymd;
}

const AGENDA_DAYS = 7;

/** The window a view shows around the day the member is looking at. */
export function windowOf(view: View, anchor: Ymd): Period {
  switch (view) {
    case "agenda": {
      return { from: anchor, to: addDays(anchor, AGENDA_DAYS) };
    }
    case "week": {
      const from = weekStart(anchor);
      return { from, to: addDays(from, 7) };
    }
    case "month": {
      const from = weekStart(monthStart(anchor));
      const last = addDays(addMonths(anchor, 1), -1);
      const to = addDays(weekStart(last), 7);
      return { from, to };
    }
  }
}

/** The day to look at after J (`1`) or K (`-1`). */
export function shift(view: View, anchor: Ymd, direction: 1 | -1): Ymd {
  return view === "month" ? addMonths(anchor, direction) : addDays(anchor, direction * 7);
}

export function daysOf(period: Period): Ymd[] {
  return Array.from({ length: daysBetween(period.from, period.to) }, (_, index) =>
    addDays(period.from, index),
  );
}

/** Today on the wall clock of `zone`, and the minutes since its midnight. */
export function nowIn(zone: string, now: Date): { day: Ymd; minute: number } {
  const parts = (timeZone: string) =>
    new Intl.DateTimeFormat("en-CA", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
    }).formatToParts(now);
  let found: Intl.DateTimeFormatPart[];
  try {
    found = parts(zone);
  } catch {
    found = parts("UTC");
  }
  const part = (type: string) => found.find((each) => each.type === type)?.value ?? "00";
  return {
    day: `${part("year")}-${part("month")}-${part("day")}`,
    minute: Number(part("hour")) * 60 + Number(part("minute")),
  };
}

/** A time zone's city, for "Lisbon time": the last part of `Europe/Lisbon`. */
export function cityOf(zone: string): string {
  return (zone.split("/").at(-1) ?? zone).replaceAll("_", " ");
}

/* Formatting, in the member's language. */

function format(locale: string, day: Ymd, options: Intl.DateTimeFormatOptions): string {
  return new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" }).format(utc(day));
}

/** "Tuesday 27 October" (the heading of a day). */
export function dayName(locale: string, day: Ymd): string {
  return format(locale, day, { weekday: "long" });
}

export function daySub(locale: string, day: Ymd): string {
  return format(locale, day, { day: "numeric", month: "long" });
}

/** "Tuesday, 27 October": a day in one phrase. */
export function dayLong(locale: string, day: Ymd): string {
  return format(locale, day, { weekday: "long", day: "numeric", month: "long" });
}

export function dayShort(locale: string, day: Ymd): string {
  return format(locale, day, { weekday: "short" });
}

export function dayNumber(day: Ymd): string {
  return String(Number(day.slice(8, 10)));
}

export function monthTitle(locale: string, day: Ymd): string {
  return format(locale, day, { month: "long", year: "numeric" });
}

/** "09:30" on the 24-hour clock. */
export function clock(locale: string, minutes: number): string {
  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC",
  }).format(new Date(Date.UTC(1970, 0, 1, 0, minutes)));
}

/** An hour of the grid: "09:00". */
export function hourLabel(locale: string, hour: number): string {
  return clock(locale, hour * 60);
}

/** The time of an event on the member's clock in another zone: "10:25". */
export function inZone(locale: string, time: string, zone: string): string | undefined {
  try {
    return new Intl.DateTimeFormat(locale, {
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
      timeZone: zone,
    }).format(new Date(time));
  } catch {
    return undefined;
  }
}

/* Events on days. */

/** Whether it stands (a cancelled event is not shown). */
export function isShown(event: EventView): boolean {
  return event.status !== "cancelled";
}

/** The last day an event covers, inclusive. */
export function lastDay(event: EventView): Ymd {
  if (event.all_day) {
    const last = addDays(dayOf(event.ends), -1);
    return last < dayOf(event.starts) ? dayOf(event.starts) : last;
  }
  const end = dayOf(event.ends);
  // Ending exactly at midnight belongs to the day before.
  return minutesOf(event.ends) === 0 && end > dayOf(event.starts) ? addDays(end, -1) : end;
}

/** An event shown across days (all-day, or a timed one lasting a day or more). */
export function spansDays(event: EventView): boolean {
  if (event.all_day) {
    return true;
  }
  const length =
    daysBetween(dayOf(event.starts), dayOf(event.ends)) * 1440 +
    minutesOf(event.ends) -
    minutesOf(event.starts);
  return length >= 1440;
}

export function startMinutes(event: EventView): number {
  return event.all_day ? 0 : minutesOf(event.starts);
}

/** The events of one day for the agenda: all-day first, then by time. */
export function agendaOn(events: readonly EventView[], day: Ymd): EventView[] {
  return events
    .filter((event) => {
      if (!isShown(event)) {
        return false;
      }
      if (event.all_day) {
        return dayOf(event.starts) <= day && day <= lastDay(event);
      }
      return dayOf(event.starts) === day;
    })
    .toSorted(
      (a, b) =>
        Number(b.all_day) - Number(a.all_day) ||
        startMinutes(a) - startMinutes(b) ||
        a.title.localeCompare(b.title),
    );
}

/** The events a week's All day lane shows, with the columns (1-based, `to` exclusive) they cover. */
export function laneOf(
  events: readonly EventView[],
  days: readonly Ymd[],
): { event: EventView; from: number; to: number }[] {
  const first = days[0];
  const last = days.at(-1);
  if (first === undefined || last === undefined) {
    return [];
  }
  return events
    .filter((event) => isShown(event) && spansDays(event))
    .toSorted((a, b) => dayOf(a.starts).localeCompare(dayOf(b.starts)))
    .flatMap((event) => {
      const start = dayOf(event.starts);
      const end = lastDay(event);
      if (end < first || start > last) {
        return [];
      }
      const from = Math.max(daysBetween(first, start), 0) + 1;
      const to = Math.min(daysBetween(first, end), days.length - 1) + 2;
      return [{ event, from, to }];
    });
}

interface Placed {
  event: EventView;
  /** Minutes from midnight, clipped to the day. */
  start: number;
  end: number;
  column: number;
  columns: number;
}

/** A day's timed events as blocks, side by side where they overlap. */
export function blocksOn(events: readonly EventView[], day: Ymd): Placed[] {
  const timed = events
    .filter((event) => isShown(event) && !spansDays(event))
    .filter((event) => dayOf(event.starts) <= day && day <= lastDay(event))
    .map((event) => {
      const start = dayOf(event.starts) < day ? 0 : minutesOf(event.starts);
      const end = lastDay(event) > day ? 1440 : minutesOf(event.ends);
      return { event, start, end: Math.max(end, start + 30) };
    })
    .toSorted((a, b) => a.start - b.start || b.end - a.end);

  const placed: Placed[] = [];
  let cluster: { event: EventView; start: number; end: number; column: number }[] = [];
  let clusterEnd = -1;
  const close = () => {
    const columns = Math.max(...cluster.map((each) => each.column), -1) + 1;
    placed.push(...cluster.map((each) => ({ ...each, columns })));
    cluster = [];
    clusterEnd = -1;
  };
  for (const item of timed) {
    if (cluster.length > 0 && item.start >= clusterEnd) {
      close();
    }
    const taken = new Set(
      cluster.filter((each) => each.end > item.start).map((each) => each.column),
    );
    let column = 0;
    while (taken.has(column)) {
      column += 1;
    }
    cluster.push({ ...item, column });
    clusterEnd = Math.max(clusterEnd, item.end);
  }
  if (cluster.length > 0) {
    close();
  }
  return placed;
}

/** The hours the week grid draws: 07:00 to 21:00, wider if an event asks for it. */
export function hoursFor(blocks: readonly Placed[]): { from: number; to: number } {
  const earliest = Math.min(...blocks.map((block) => block.start), 7 * 60);
  const latest = Math.max(...blocks.map((block) => block.end), 21 * 60);
  return { from: Math.floor(earliest / 60), to: Math.min(Math.ceil(latest / 60), 24) };
}

/** An event's own place in a month's cells: up to three shown, the rest as "+N". */
export const MONTH_SHOWN = 3;

/** Whether the member could add this event to a provider calendar of theirs. */
export function canPush(
  event: EventView,
  calendars: readonly { id: string; source: string; writable: boolean }[],
): boolean {
  const own = calendars.find((calendar) => calendar.id === event.calendar_id);
  return (
    own?.source === "campbooks" &&
    !event.on_provider &&
    calendars.some((calendar) => calendar.source === "provider" && calendar.writable)
  );
}

/* Writing times back to the server. */

/** `+01:00`: the offset a zone has at a moment, for writing a wall-clock time as RFC 3339. */
export function zoneOffset(zone: string, at: Date): string {
  try {
    const name = new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      timeZoneName: "longOffset",
    })
      .formatToParts(at)
      .find((part) => part.type === "timeZoneName")?.value;
    const match = /GMT([+-]\d{2}:\d{2})/u.exec(name ?? "");
    return match?.[1] ?? "+00:00";
  } catch {
    return "+00:00";
  }
}

/** `2026-11-06T07:40:00+00:00` for a wall-clock `time` (`HH:MM`) on `day` in `zone`. */
export function atWallClock(zone: string, day: Ymd, time: string): string {
  const guess = new Date(`${day}T${time}:00Z`);
  return `${day}T${time}:00${zoneOffset(zone, guess)}`;
}

/** `HH:MM` of an event time, for a time field. */
export function timeField(time: string): string {
  return time.length < 16 ? "" : time.slice(11, 16);
}

/** The time of day a task is due at, in minutes, if it is due at one. */
export function dueMinutes(item: { due: string }): number | undefined {
  return item.due.length > 10 ? minutesOf(item.due) : undefined;
}
