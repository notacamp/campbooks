// Times of day as "HH:MM" on a 24-hour clock: the time picker's slots and
// the grammar of a time the member types ("9", "930", "9:30", "21h15", "9pm").

/** A time of day: "09:30". */
export type Time = string;

const MINUTES_PER_DAY = 24 * 60;

const pad = (n: number) => String(n).padStart(2, "0");

/** Minutes since midnight. */
export function toMinutes(time: Time): number {
  return Number(time.slice(0, 2)) * 60 + Number(time.slice(3, 5));
}

/** "HH:MM" for minutes since midnight. */
export function fromMinutes(minutes: number): Time {
  return `${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
}

/** Whether `value` is a whole "HH:MM" time. */
export function isTime(value: string): boolean {
  const match = /^(\d{2}):(\d{2})$/u.exec(value);
  return match !== null && Number(match[1]) < 24 && Number(match[2]) < 60;
}

// "9", "09", "9:30", "9.30", "9h30", "21 h 15", with an optional am/pm.
const CLOCK = /^(\d{1,2})(?:\s*[:.h]\s*(\d{2})?)?\s*([ap])?\.?\s*(?:m\.?)?$/u;
// "930", "0930", "2115", with an optional am/pm.
const DIGITS = /^(\d{3,4})\s*([ap])?\.?\s*(?:m\.?)?$/u;

/**
 * The time a member typed, or nothing when it is no time. Reads hours alone
 * ("9"), hours and minutes with `:`, `.` or `h`, three or four digits
 * ("930"), and am/pm ("9pm", "9:30 a.m."). This is the member's own input,
 * not message content.
 */
export function parseTime(typed: string): Time | undefined {
  const text = typed
    .trim()
    .toLowerCase()
    .replaceAll(/[\u00A0\u202F]/gu, " ");
  let hours: number;
  let minutes: number;
  let meridiem: string | undefined;
  const clock = CLOCK.exec(text);
  const digits = clock === null ? DIGITS.exec(text) : null;
  if (clock !== null) {
    hours = Number(clock[1]);
    minutes = clock[2] === undefined ? 0 : Number(clock[2]);
    meridiem = clock[3];
  } else if (digits !== null) {
    const all = digits[1] ?? "";
    hours = Number(all.slice(0, -2));
    minutes = Number(all.slice(-2));
    meridiem = digits[2];
  } else {
    return undefined;
  }
  if (minutes > 59) {
    return undefined;
  }
  if (meridiem !== undefined) {
    if (hours < 1 || hours > 12) {
      return undefined;
    }
    hours = (hours % 12) + (meridiem === "p" ? 12 : 0);
  }
  return hours > 23 ? undefined : fromMinutes(hours * 60 + minutes);
}

/** Every `step` minutes of the day from midnight, from `min` on when given. */
export function timeSlots(step: number, min?: Time): Time[] {
  const every = Math.max(1, Math.floor(step));
  const from = min === undefined ? 0 : toMinutes(min);
  const slots: Time[] = [];
  for (let minutes = 0; minutes < MINUTES_PER_DAY; minutes += every) {
    if (minutes >= from) {
      slots.push(fromMinutes(minutes));
    }
  }
  return slots;
}

/** The index of the slot closest to `time` (the first one on a tie), or -1 with no slots. */
export function nearestSlot(slots: readonly Time[], time: Time): number {
  const target = toMinutes(time);
  let best = -1;
  let distance = Number.POSITIVE_INFINITY;
  for (const [index, slot] of slots.entries()) {
    const gap = Math.abs(toMinutes(slot) - target);
    if (gap < distance) {
      best = index;
      distance = gap;
    }
  }
  return best;
}

/** The time as the member's language writes it: "9:30 AM" in English, "09:30" in Portuguese. */
export function formatTime(time: Time, locale: string): string {
  const minutes = toMinutes(time);
  return new Intl.DateTimeFormat(locale, {
    hour: "numeric",
    minute: "2-digit",
    timeZone: "UTC",
  }).format(new Date(Date.UTC(1970, 0, 1, Math.floor(minutes / 60), minutes % 60)));
}
