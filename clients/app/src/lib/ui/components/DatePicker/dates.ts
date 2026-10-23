// Whole days as "YYYY-MM-DD", with no time and no zone: the date picker's
// arithmetic. Counted in UTC, so a daylight-saving change never moves a day.

/** A whole day: "2026-10-26". */
export type Day = string;

/** A weekday as `Intl.Locale` week info counts it: 1 is Monday, 7 is Sunday. */
export type Weekday = 1 | 2 | 3 | 4 | 5 | 6 | 7;

const DAY = /^(\d{4})-(\d{2})-(\d{2})$/u;
const MS_PER_DAY = 86_400_000;

/** The day's year, month (1-12) and date, or nothing for a string that is no real day. */
export function parseDay(day: string): { year: number; month: number; date: number } | undefined {
  const match = DAY.exec(day);
  if (match === null) {
    return undefined;
  }
  const [year, month, date] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const at = new Date(Date.UTC(year, month - 1, date));
  return at.getUTCFullYear() === year && at.getUTCMonth() === month - 1 && at.getUTCDate() === date
    ? { year, month, date }
    : undefined;
}

const pad = (n: number, width = 2) => String(n).padStart(width, "0");

/** The day at a UTC instant. */
function fromUtc(at: Date): Day {
  return `${pad(at.getUTCFullYear(), 4)}-${pad(at.getUTCMonth() + 1)}-${pad(at.getUTCDate())}`;
}

/** Noon UTC of `day`: the instant to format it with `timeZone: "UTC"`. */
export function dayToDate(day: Day): Date {
  const parts = parseDay(day);
  if (parts === undefined) {
    throw new RangeError(`not a day: ${day}`);
  }
  return new Date(Date.UTC(parts.year, parts.month - 1, parts.date, 12));
}

/** The day `count` days after `day` (before, when negative). */
export function addDays(day: Day, count: number): Day {
  return fromUtc(new Date(dayToDate(day).getTime() + count * MS_PER_DAY));
}

/** The same date `count` months later, on the month's last day when it is shorter. */
export function addMonths(day: Day, count: number): Day {
  const at = dayToDate(day);
  const date = at.getUTCDate();
  const first = new Date(Date.UTC(at.getUTCFullYear(), at.getUTCMonth() + count, 1, 12));
  const last = new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + 1, 0, 12));
  first.setUTCDate(Math.min(date, last.getUTCDate()));
  return fromUtc(first);
}

/** The weekday of `day`, Monday 1 to Sunday 7. */
export function weekdayOf(day: Day): Weekday {
  const sunday = dayToDate(day).getUTCDay();
  return (sunday === 0 ? 7 : sunday) as Weekday;
}

/** The first day of `day`'s week, for a week that starts on `firstDay`. */
export function startOfWeek(day: Day, firstDay: Weekday): Day {
  return addDays(day, -((weekdayOf(day) - firstDay + 7) % 7));
}

/** The last day of `day`'s week. */
export function endOfWeek(day: Day, firstDay: Weekday): Day {
  return addDays(startOfWeek(day, firstDay), 6);
}

/** The first of the month `day` is in. */
export function startOfMonth(day: Day): Day {
  return `${day.slice(0, 8)}01`;
}

/**
 * The month of `day` as six weeks of seven days, from the start of the week
 * the first falls in: the days before and after the month fill the grid.
 */
export function monthMatrix(day: Day, firstDay: Weekday): Day[][] {
  const start = startOfWeek(startOfMonth(day), firstDay);
  return Array.from({ length: 6 }, (_, week) =>
    Array.from({ length: 7 }, (_, weekday) => addDays(start, week * 7 + weekday)),
  );
}

/** Whether two days are in the same month. */
export function sameMonth(a: Day, b: Day): boolean {
  return a.slice(0, 7) === b.slice(0, 7);
}

/** `day`, moved inside `[min, max]` when it falls outside. */
export function clampDay(day: Day, min?: Day, max?: Day): Day {
  if (min !== undefined && day < min) {
    return min;
  }
  if (max !== undefined && day > max) {
    return max;
  }
  return day;
}

/** Whether `day` is inside `[min, max]`. */
export function inRange(day: Day, min?: Day, max?: Day): boolean {
  return (min === undefined || day >= min) && (max === undefined || day <= max);
}

/** Today on this device's calendar. */
export function todayOf(now: Date): Day {
  return `${pad(now.getFullYear(), 4)}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

interface WeekInfoLocale {
  getWeekInfo?: () => { firstDay?: number };
  weekInfo?: { firstDay?: number };
}

/** The day a week starts on in `locale`, Monday where the platform does not say. */
export function weekStart(locale: string): Weekday {
  try {
    const info = new Intl.Locale(locale) as unknown as WeekInfoLocale;
    const firstDay = info.getWeekInfo?.().firstDay ?? info.weekInfo?.firstDay;
    return firstDay !== undefined && firstDay >= 1 && firstDay <= 7 ? (firstDay as Weekday) : 1;
  } catch {
    return 1;
  }
}
