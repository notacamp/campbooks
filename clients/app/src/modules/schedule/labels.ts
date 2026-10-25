import type { TFunction } from "i18next";

import type { CalendarView, EventView } from "./api/types";
import type { TaskRow } from "./due";
import {
  cityOf,
  clock,
  dayLong,
  dayOf,
  dayShort,
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

/** Whose calendar it is, as the member knows it ("Google"); none when the server did not say. */
export function providerName(
  calendar: Pick<CalendarView, "provider"> | undefined,
): string | undefined {
  switch (calendar?.provider) {
    case "google":
      return "Google";
    case "zoho":
      return "Zoho";
    default:
      return undefined;
  }
}

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

/** "Pay · EDP bill", or "Do · Send the lease". */
export function dueTitle(t: T, row: TaskRow): string {
  return t(row.task.verb === "pay" ? "due.pay" : "due.do", { title: row.task.title });
}

/** "Due today", "Due 17:00", "Overdue · 2 days", "Planned", "Planned Fri". */
export function dueWhen(t: T, locale: string, row: TaskRow, today: Ymd): string {
  if (row.kind === "planned") {
    return row.late > 0
      ? t("due.plannedOn", { day: dayShort(locale, row.task.plan_on ?? today) })
      : t("due.planned");
  }
  if (row.late > 0) {
    return t("due.overdue", { count: row.late });
  }
  if (row.minute !== undefined) {
    return t("due.at", { time: clock(locale, row.minute) });
  }
  return row.stands === today ? t("due.today") : t("due.label");
}

/** Whether `dueWhen` says only the row's kind again ("Due", "Planned") and not a day or time. */
export function dueWhenIsKind(row: TaskRow, today: Ymd): boolean {
  return (
    row.late === 0 && (row.kind === "planned" || (row.minute === undefined && row.stands !== today))
  );
}

/**
 * The rest of a row's second line: a Planned row says when it is due ("Due
 * Thu"); a Due row says "Planned" when it is also planned for its day; else "Task".
 */
export function dueMeta(t: T, locale: string, row: TaskRow): string {
  const due = row.task.due_on ?? undefined;
  if (row.kind === "planned" && due !== undefined) {
    return t("due.dueOn", { day: dayShort(locale, due) });
  }
  return t(row.alsoPlanned ? "due.metaPlanned" : "due.meta");
}
