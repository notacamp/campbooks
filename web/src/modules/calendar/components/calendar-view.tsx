/**
 * CalendarView — read-only agenda grouped by day.
 *
 * Consumes `TimeAgenda` from `GET /api/app/time`, rendering:
 *   - Days (Today / named weekdays) with events, deadlines, and focus blocks
 *   - Scout day note for today (Visor + short summary)
 *   - Colour-ticked blocks: event = calendar/people colour, deadline = now,
 *     focus block = time (dashed/outlined)
 *   - All-day items shown at the top of each day group
 *
 * Placement note: this is a view-layer component placed in modules/ by
 * design. The eslint-disable below suppresses the modules HTML restriction
 * that is intended for business-logic components, not layout primitives.
 *
 * Read-only display — no CRUD actions.
 */

/* eslint-disable react/forbid-elements */

import {
  forwardRef,
  useMemo,
  type HTMLAttributes,
  type CSSProperties,
  type JSX,
} from "react";
import { cn } from "~/lib/utils";
import { Visor } from "~/lib/ui/visor/visor";
import styles from "./calendar-view.module.css";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface AgendaItemAction {
  kind: string;
  label: string;
  primary: boolean;
}

export interface TaskExtra {
  id: number;
  status: string;
  priority: string;
  ai_suggested: boolean;
  snoozed_until: string | null;
}

export interface ReminderExtra {
  id: number;
  status: string;
  reminder_type: string;
  snoozed_until: string | null;
}

export interface EventExtra {
  id: number;
  provider_event_id: string;
  join_url: string | null;
  rsvp_status: string | null;
  calendar_id: number;
}

export interface FocusBlockExtra {
  id: number;
  status: string;
  task_id: number | null;
  calendar_event_id: number | null;
}

/** Serialized by Api::App::Time::AgendaItemSerializer */
export interface AgendaItem {
  kind: "event" | "task" | "deadline" | "focus";
  id: number | null;
  title: string;
  /** ISO8601 datetime — null for all-day items */
  at: string | null;
  /** YYYY-MM-DD */
  day: string | null;
  all_day: boolean;
  overdue: boolean;
  duration_minutes: number | null;
  /**
   * Hex colour from the calendar provider (e.g. a Google Calendar event colour).
   * Passed as a CSS custom property value — API data, not a design-system literal.
   */
  color: string | null;
  source_label: string | null;
  source_path: string | null;
  emphasis: boolean | null;
  why: string | null;
  prep_name: string | null;
  prep_detail: string | null;
  handed: boolean | null;
  actions: AgendaItemAction[] | null;
  task?: TaskExtra;
  reminder?: ReminderExtra;
  event?: EventExtra;
  focus_block?: FocusBlockExtra;
}

export interface DayNoteFocus {
  title: string | null;
  subject: string | null;
  at: string | null;
  duration_minutes: number | null;
}

export interface LateObligation {
  name: string;
  days_overdue: number;
}

export interface PrepMeeting {
  title: string | null;
  at: string | null;
  first_name: string | null;
  why: string | null;
}

/** Serialized by Api::App::Time::AgendaSerializer#serialize_day_note */
export interface DayNote {
  /** YYYY-MM-DD */
  date: string | null;
  meetings_count: number;
  deadlines_count: number;
  first_deadline_title: string | null;
  undated_count: number;
  focus: DayNoteFocus | null;
  late_obligation: LateObligation | null;
  prep_meeting: PrepMeeting | null;
}

/** Serialized by Api::App::Time::AgendaSerializer#serialize_suggestion */
export interface FocusSuggestion {
  starts_at: string | null;
  ends_at: string | null;
  duration_minutes: number | null;
}

/** Full response shape from GET /api/app/time */
export interface TimeAgenda {
  items: AgendaItem[];
  undated: AgendaItem[];
  day_note: DayNote | null;
  suggestions: FocusSuggestion[];
}

export interface CalendarViewProps extends HTMLAttributes<HTMLDivElement> {
  /** Full agenda from GET /api/app/time */
  data: TimeAgenda;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** YYYY-MM-DD for today in local time (avoids UTC/local boundary mismatch). */
const localTodayIso = (): string => {
  const d = new Date();
  return [
    String(d.getFullYear()),
    String(d.getMonth() + 1).padStart(2, "0"),
    String(d.getDate()).padStart(2, "0"),
  ].join("-");
};

/** Parse a YYYY-MM-DD string as local-midnight (not UTC midnight). */
const parseDateLocal = (dateStr: string): Date => {
  const parts = dateStr.split("-").map(Number);
  return new Date(parts[0] ?? 0, (parts[1] ?? 1) - 1, parts[2] ?? 1);
};

interface DayLabel {
  heading: string;
  sub: string;
  isToday: boolean;
}

const formatDayLabel = (dateStr: string, todayStr: string): DayLabel => {
  const isToday = dateStr === todayStr;
  const d = parseDateLocal(dateStr);
  const weekday = d.toLocaleDateString("en-GB", { weekday: "long" });
  const dayNum = d.getDate();
  const month = d.toLocaleDateString("en-GB", { month: "long" });
  return {
    heading: isToday ? "Today" : weekday,
    sub: isToday ? `${weekday} ${dayNum}` : `${dayNum} ${month}`,
    isToday,
  };
};

/** "09:30" or "all day". */
const formatTime = (at: string | null, allDay: boolean): string => {
  if (allDay) return "all day";
  if (!at) return "";
  const d = new Date(at);
  return d.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
};

/**
 * Resolve the left-tick CSS colour for a block.
 *
 * Events carry a `color` hex from the provider (e.g. Google Calendar colour).
 * This value is passed as a CSS custom property value — it is API data, not
 * a hardcoded design-system literal. Other kinds use semantic signal tokens.
 */
const tickColor = (item: AgendaItem): string => {
  switch (item.kind) {
    case "event":
      return item.color ?? "var(--cb-people)";
    case "deadline":
      return "var(--cb-now)";
    case "focus":
    case "task":
      return "var(--cb-time)";
  }
};

/** Sub-line text for a block. */
const subLine = (item: AgendaItem): string | null =>
  item.source_label ?? item.why ?? null;

/** Group dated items by YYYY-MM-DD, preserving server order within each group. */
const groupByDay = (items: AgendaItem[]): Map<string, AgendaItem[]> => {
  const map = new Map<string, AgendaItem[]>();
  for (const item of items) {
    const key = item.day ?? "undated";
    const bucket = map.get(key);
    if (bucket) {
      bucket.push(item);
    } else {
      map.set(key, [item]);
    }
  }
  return map;
};

/** All-day items first, then sorted by start time. */
const sortDay = (items: AgendaItem[]): AgendaItem[] =>
  [...items].sort((a, b) => {
    if (a.all_day && !b.all_day) return -1;
    if (!a.all_day && b.all_day) return 1;
    if (!a.at || !b.at) return 0;
    return a.at < b.at ? -1 : a.at > b.at ? 1 : 0;
  });

const pl = (n: number, one: string, many: string): string =>
  n === 1 ? one : many;

const composeDayNoteText = (note: DayNote): string => {
  const parts: string[] = [];

  if (note.meetings_count > 0) {
    parts.push(
      `${note.meetings_count} ${pl(note.meetings_count, "meeting", "meetings")}`,
    );
  }

  if (note.deadlines_count > 0) {
    if (note.first_deadline_title && note.deadlines_count === 1) {
      parts.push(note.first_deadline_title);
    } else if (note.first_deadline_title && note.deadlines_count > 1) {
      const rest = note.deadlines_count - 1;
      parts.push(
        `${note.first_deadline_title} and ${rest} other ${pl(rest, "deadline", "deadlines")}`,
      );
    } else {
      parts.push(
        `${note.deadlines_count} ${pl(note.deadlines_count, "deadline", "deadlines")}`,
      );
    }
  }

  if (note.focus) {
    // The API's focus title already carries a "Focus: " prefix; prefer the
    // clean subject so we don't render "Focus: Focus: …".
    const subject =
      note.focus.subject ??
      note.focus.title?.replace(/^focus:\s*/i, "") ??
      null;
    if (subject) parts.push(`Focus: ${subject}`);
  }

  if (note.late_obligation) {
    const { name, days_overdue } = note.late_obligation;
    parts.push(
      `${name} is ${days_overdue} ${pl(days_overdue, "day", "days")} overdue`,
    );
  }

  return parts.join(" · ");
};

// ── Sub-components ────────────────────────────────────────────────────────────

const DayNoteRow = ({ note }: { note: DayNote }): JSX.Element => {
  const text = composeDayNoteText(note);
  if (!text) return <></>;
  return (
    <div className={styles.dayNote} role="note">
      <Visor size={18} state="reading" aria-hidden />
      <p className={styles.dayNoteText}>{text}</p>
    </div>
  );
};

const AgendaBlock = ({ item }: { item: AgendaItem }): JSX.Element => {
  const color = tickColor(item);
  const isFocus = item.kind === "focus";
  const sub = subLine(item);

  // --cal-ev-color is read by the ::before pseudo in the CSS module.
  const blockStyle = { "--cal-ev-color": color } as CSSProperties;

  return (
    <div
      className={cn(
        styles.block,
        isFocus && styles.blockFocus,
        item.overdue && styles.blockOverdue,
      )}
      style={blockStyle}
      aria-label={item.overdue ? `${item.title} (overdue)` : undefined}
    >
      <span className={styles.blockTitle}>{item.title}</span>
      {sub && <span className={styles.blockSub}>{sub}</span>}
    </div>
  );
};

const AgendaRow = ({ item }: { item: AgendaItem }): JSX.Element => {
  const timeStr = formatTime(item.at, item.all_day);
  const dateTimeAttr = item.all_day ? (item.day ?? undefined) : (item.at ?? undefined);

  return (
    <li className={styles.row}>
      <time
        className={styles.time}
        dateTime={dateTimeAttr}
        aria-label={item.all_day ? "All day" : timeStr}
      >
        {timeStr}
      </time>
      <AgendaBlock item={item} />
    </li>
  );
};

interface DayGroupProps {
  dateStr: string;
  items: AgendaItem[];
  dayNote: DayNote | null;
  todayStr: string;
  headingId: string;
}

const DayGroup = ({
  dateStr,
  items,
  dayNote,
  todayStr,
  headingId,
}: DayGroupProps): JSX.Element => {
  const { heading, sub, isToday } = formatDayLabel(dateStr, todayStr);
  const sorted = useMemo(() => sortDay(items), [items]);
  const showNote =
    isToday && dayNote !== null && dayNote.date === dateStr;

  return (
    <section aria-labelledby={headingId} className={styles.day}>
      <header className={styles.dayHeader}>
        <h3 id={headingId} className={styles.dayHeading}>
          {heading}
        </h3>
        <span className={styles.daySub} aria-hidden>
          {sub}
        </span>
      </header>

      {showNote && dayNote && <DayNoteRow note={dayNote} />}

      <ol
        className={styles.dayItems}
        aria-label={`Events for ${heading}`}
      >
        {sorted.map((item, idx) => (
          <AgendaRow
            key={
              item.id !== null
                ? `${item.kind}-${item.id}`
                : `${item.kind}-${idx}`
            }
            item={item}
          />
        ))}
      </ol>
    </section>
  );
};

// ── Main component ────────────────────────────────────────────────────────────

export const CalendarView = forwardRef<HTMLDivElement, CalendarViewProps>(
  ({ data, className, ...props }, ref) => {
    const todayStr = useMemo(() => localTodayIso(), []);

    const grouped = useMemo(
      () => groupByDay(data.items.filter((i) => i.day !== null)),
      [data.items],
    );

    const isEmpty = grouped.size === 0;

    return (
      <div
        ref={ref}
        role="region"
        aria-label="Calendar agenda"
        className={cn(styles.agenda, className)}
        {...props}
      >
        {isEmpty ? (
          <p className={styles.empty}>
            Nothing on your calendar this week.
          </p>
        ) : (
          Array.from(grouped.entries()).map(([dateStr, items], groupIdx) => (
            <DayGroup
              key={dateStr}
              dateStr={dateStr}
              items={items}
              dayNote={data.day_note}
              todayStr={todayStr}
              headingId={`cal-day-${groupIdx}`}
            />
          ))
        )}
      </div>
    );
  },
);

CalendarView.displayName = "CalendarView";
