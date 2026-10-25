import type { TaskView } from "./api/queries";
import { addDays, isoDay } from "./model";

/** A free stretch of the member's calendar, as the module that keeps calendars answers it. */
export interface FreeSlot {
  /** RFC 3339. */
  starts: string;
  ends: string;
}

/** A calendar a hold can go on. */
export interface HoldCalendar {
  id: string;
  name: string;
  /** The member's own Campbooks calendar, which no provider sees. */
  campbooks: boolean;
}

/** What `useFreeSlots` answers. */
export interface FreeSlots {
  /** Earliest first; `undefined` while they load. */
  slots: readonly FreeSlot[] | undefined;
  /** They could not be read. */
  failed: boolean;
}

/** The window to look for free time in, and the length of slot. */
export interface FreeWindow {
  /** RFC 3339. */
  from: string;
  to: string;
  minutes: number;
}

/**
 * Where Hold time reads the member's calendar: the schedule module's free
 * slots and writable calendars (the primary provider calendar first),
 * plugged in by the app so this module never imports it.
 */
export interface HoldSources {
  useFreeSlots: (workspaceId: string, window: FreeWindow) => FreeSlots;
  useWritableCalendars: (workspaceId: string) => readonly HoldCalendar[] | undefined;
}

/** The lengths Hold time offers, in minutes; 30 by default. */
export const HOLD_LENGTHS = [15, 30, 60] as const;
export type HoldLength = (typeof HOLD_LENGTHS)[number];
export const DEFAULT_LENGTH: HoldLength = 30;

/** How many slots it offers. */
const OFFERED = 3;

const MINUTE_MS = 60_000;
/** Starts fall on a five-minute mark, a moment after now. */
const STEP_MS = 5 * MINUTE_MS;

/** A slot picked: when, and on which calendar. */
export interface HoldPick {
  starts: string;
  ends: string;
  /** `undefined`: the member's Campbooks calendar. */
  calendarId: string | undefined;
}

/**
 * Where to look: from now to the end of the due day, or the next 7 days when
 * the task has no due day or it has passed.
 */
export function holdWindow(task: TaskView, now: Date, minutes: number): FreeWindow {
  const today = isoDay(now);
  const due = task.due_on ?? undefined;
  let end = addDays(now, 7);
  if (due !== undefined && due >= today) {
    const [year, month, day] = due.split("-").map(Number);
    end = new Date(year ?? 0, (month ?? 1) - 1, (day ?? 1) + 1, 0, 0, 0, 0);
  }
  return { from: now.toISOString(), to: end.toISOString(), minutes };
}

/** Whether the window ends with the task's due day (not the next 7 days). */
export function untilDue(task: TaskView, now: Date): boolean {
  const due = task.due_on ?? undefined;
  return due !== undefined && due >= isoDay(now);
}

/**
 * The first three slots of `minutes` in `free`: the start of each free
 * stretch first, then later starts within a stretch, a length apart, so
 * three appear even with one long free afternoon. Earliest first.
 */
export function holdSlots(free: readonly FreeSlot[], minutes: number, now: Date): FreeSlot[] {
  const length = minutes * MINUTE_MS;
  const earliest = Math.ceil((now.getTime() + MINUTE_MS) / STEP_MS) * STEP_MS;
  const stretches = free.map((slot) => {
    const start = Math.max(new Date(slot.starts).getTime(), earliest);
    return { start: Math.ceil(start / STEP_MS) * STEP_MS, end: new Date(slot.ends).getTime() };
  });
  const firsts = stretches.filter((s) => s.start + length <= s.end).map((s) => s.start);
  const picked = firsts.slice(0, OFFERED);
  for (const stretch of stretches) {
    for (
      let start = stretch.start + length;
      picked.length < OFFERED && start + length <= stretch.end;
      start += length
    ) {
      picked.push(start);
    }
  }
  return picked
    .toSorted((a, b) => a - b)
    .slice(0, OFFERED)
    .map((start) => ({
      starts: new Date(start).toISOString(),
      ends: new Date(start + length).toISOString(),
    }));
}

/** "Wed 10:00–10:30" in `locale`, on the member's clock. */
export function heldLabel(hold: { starts: string; ends: string }, locale: string): string {
  const starts = new Date(hold.starts);
  const weekday = new Intl.DateTimeFormat(locale, { weekday: "short" }).format(starts);
  const time = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" });
  return `${weekday} ${time.format(starts)}–${time.format(new Date(hold.ends))}`;
}

/** Whether Hold time is offered for `task`: an open Do task (a bill is paid, not worked on). */
export function holdable(task: TaskView): boolean {
  return task.state === "open" && task.verb === "do";
}
