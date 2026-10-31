// When a send scheduled for later leaves (campbooks-cloud#392): the times
// Send's menu offers, in the device's time zone, and whether a picked time
// is one the server takes. Pure helpers, no React.

/** The furthest ahead a send may be scheduled, in days (the server's limit). */
const MAX_SEND_AHEAD_DAYS = 366;

/** How soon a picked time may be, in minutes: past the undo window and a minute. */
const SOONEST_MINUTES = 2;

/** The hour the morning choices leave at. */
const MORNING_HOUR = 8;

/** `days` after `now`'s day, at 08:00 local time (a DST change keeps 08:00). */
function morning(now: Date, days: number): Date {
  const at = new Date(now);
  at.setDate(at.getDate() + days);
  at.setHours(MORNING_HOUR, 0, 0, 0);
  return at;
}

/** Tomorrow at 08:00. */
export function tomorrowMorning(now: Date): Date {
  return morning(now, 1);
}

/** 08:00 on the next day that falls Monday to Friday. */
export function nextWorkingMorning(now: Date): Date {
  // Sunday 0 … Saturday 6: Friday, Saturday and Sunday go to Monday.
  const ahead: Record<number, number> = { 5: 3, 6: 2, 0: 1 };
  return morning(now, ahead[now.getDay()] ?? 1);
}

/** Why a picked time can't be used, or `undefined` when it can. */
export function sendTimeProblem(at: Date, now: Date): "past" | "tooFar" | undefined {
  if (Number.isNaN(at.getTime()) || at.getTime() < now.getTime() + SOONEST_MINUTES * 60_000) {
    return "past";
  }
  if (at.getTime() > now.getTime() + MAX_SEND_AHEAD_DAYS * 86_400_000) {
    return "tooFar";
  }
  return undefined;
}

/** A local date (`YYYY-MM-DD`) and time (`HH:MM`), as the pickers give them. */
export function fromPicked(date: string, time: string): Date {
  const [year, month, day] = date.split("-").map(Number);
  const [hours, minutes] = time.split(":").map(Number);
  if ([year, month, day, hours, minutes].some((n) => n === undefined || Number.isNaN(n))) {
    return new Date(Number.NaN);
  }
  return new Date(year ?? 0, (month ?? 1) - 1, day ?? 1, hours ?? 0, minutes ?? 0);
}

/** "Tue 29 Oct, 08:00" in `locale`. */
export function sendTimeLabel(at: Date | string, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "numeric",
    minute: "2-digit",
  }).format(typeof at === "string" ? new Date(at) : at);
}
