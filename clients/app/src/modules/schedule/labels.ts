import type { TFunction } from "i18next";

import type { CalendarView, EventView } from "./api/types";
import type { DueItem } from "./due";
import {
  cityOf,
  clock,
  dayLong,
  dayOf,
  daysBetween,
  inZone,
  lastDay,
  minutesOf,
  spansDays,
  type Ymd,
} from "./model";

export type T = TFunction<"schedule">;

/** One event or occurrence: its id, plus the occurrence's name for one of a series. */
export function eventKey(event: Pick<EventView, "id" | "occurrence">): string {
  return event.occurrence === undefined || event.occurrence === null
    ? event.id
    : `${event.id}~${event.occurrence}`;
}

export const dueKeyOf = (item: Pick<DueItem, "id">) => `due:${item.id}`;

/** "09:30 – 10:00", "All day", "From 15:00": the short time of an event. */
export function whenLabel(t: T, locale: string, event: EventView): string {
  if (event.all_day) {
    return t("when.allDay");
  }
  if (spansDays(event)) {
    return t("when.from", { time: clock(locale, minutesOf(event.starts)) });
  }
  return t("when.range", {
    start: clock(locale, minutesOf(event.starts)),
    end: clock(locale, minutesOf(event.ends)),
  });
}

/** "Tuesday 27 October, 15:00 – 15:45": the long time of an event. */
export function whenLong(t: T, locale: string, event: EventView): string {
  const first = dayOf(event.starts);
  if (event.all_day) {
    const last = lastDay(event);
    return last > first
      ? t("when.range", { start: dayLong(locale, first), end: dayLong(locale, last) })
      : dayLong(locale, first);
  }
  const start = `${dayLong(locale, first)}, ${clock(locale, minutesOf(event.starts))}`;
  const lastDayShown = dayOf(event.ends);
  const sameDay = lastDayShown === first;
  const end = sameDay
    ? clock(locale, minutesOf(event.ends))
    : `${dayLong(locale, lastDayShown)}, ${clock(locale, minutesOf(event.ends))}`;
  return t("when.range", { start, end });
}

/**
 * When the event was written in another zone than the member's: its times
 * as that zone's clock, "10:25 Amsterdam time". Undefined when it is the
 * same zone, or none was given.
 */
export function zoneNote(
  t: T,
  locale: string,
  event: EventView,
  memberZone: string,
): string | undefined {
  const zone = event.time_zone;
  if (event.all_day || zone === undefined || zone === null || zone === "" || zone === memberZone) {
    return undefined;
  }
  const start = inZone(locale, event.starts, zone);
  const end = inZone(locale, event.ends, zone);
  if (start === undefined || end === undefined) {
    return undefined;
  }
  return t("sheet.inZone", {
    time: t("when.range", { start, end }),
    city: cityOf(zone),
  });
}

/** The second line of a row: the place, else the calendar's name. */
export function metaLine(event: EventView, calendars: readonly CalendarView[]): string {
  const place = event.place?.trim();
  if (place !== undefined && place !== "") {
    return place;
  }
  return calendars.find((calendar) => calendar.id === event.calendar_id)?.name ?? "";
}

/** "€84.20": an amount in minor units, in the member's language. */
export function money(locale: string, amount: { minor: number; currency: string }): string {
  try {
    return new Intl.NumberFormat(locale, { style: "currency", currency: amount.currency }).format(
      amount.minor / 100,
    );
  } catch {
    return `${(amount.minor / 100).toFixed(2)} ${amount.currency}`;
  }
}

/** "Pay · EDP bill · €84.20", or "Do · Send the signed lease". */
export function dueTitle(t: T, locale: string, item: DueItem): string {
  if (item.verb === "pay") {
    return item.amount === undefined || item.amount === null
      ? t("due.pay", { title: item.title })
      : t("due.payAmount", { title: item.title, amount: money(locale, item.amount) });
  }
  return t("due.do", { title: item.title });
}

/** The day a task is due. */
function dueDay(item: DueItem): Ymd {
  return dayOf(item.due);
}

/** Whether the task is due at a time of day, not just on a date. */
function dueAtTime(item: DueItem): boolean {
  return item.due.length > 10;
}

/** Days past due as of `today` (0 when it is not). */
export function overdueDays(item: DueItem, today: Ymd): number {
  return Math.max(daysBetween(dueDay(item), today), 0);
}

/** The day a Due row stands on: its day, or today once it is overdue. */
export function dueStandsOn(item: DueItem, today: Ymd): Ymd {
  return dueDay(item) < today ? today : dueDay(item);
}

/** "Due today", "Due 17:00", "Overdue · 2 days". */
export function dueWhen(t: T, locale: string, item: DueItem, today: Ymd): string {
  const late = overdueDays(item, today);
  if (late > 0) {
    return t("due.overdue", { count: late });
  }
  if (dueAtTime(item)) {
    return t("due.at", { time: clock(locale, minutesOf(item.due)) });
  }
  return dueDay(item) === today ? t("due.today") : t("due.label");
}
