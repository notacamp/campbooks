import { useState, type ComponentPropsWithRef, type CSSProperties, type ReactNode } from "react";

import type { ShortcutId } from "~/lib/shortcuts";

import { cx } from "../../internal/cx";
import type { CalendarKind } from "../../tokens/tokens";
import { Icon, type IconName } from "../Icon/Icon";
import { KbdHint } from "../Kbd/Kbd";
import { Heading, VisuallyHidden } from "../Typography/Typography";
import { Visor } from "../Visor/Visor";

import styles from "./Calendar.module.css";

/** The icon each kind of dated thing is drawn with. */
export const KIND_ICONS: Record<CalendarKind, IconName> = {
  charge: "card",
  appointment: "clock",
  meeting: "people",
  travel: "travel",
  stay: "stay",
  show: "ticket",
  reservation: "dining",
  delivery: "parcel",
  expiry: "hourglass",
  due: "task",
  hold: "clock",
  none: "schedule",
};

/** CSS custom properties a block is placed with (unitless numbers, read by the stylesheet). */
const place = (values: Record<string, number>) => values as unknown as CSSProperties;

/**
 * A kind's icon in its colour, on a soft square. `md` takes an avatar's
 * place at the start of a row (an event among Home's items).
 */
export function KindIcon({ kind, size = "sm" }: { kind: CalendarKind; size?: "sm" | "md" }) {
  return (
    <span className={styles["kindIcon"]} data-kind={kind} data-size={size} aria-hidden="true">
      <Icon name={KIND_ICONS[kind]} />
    </span>
  );
}

export interface KindDotProps {
  kind: CalendarKind;
  /** A ring rather than a dot: on no calendar of the member's provider, only they see it. */
  hollow?: boolean | undefined;
  /** Says what the ring means, for those who cannot see it; none for a filled dot. */
  label?: string | undefined;
}

/** A kind as a small dot in its colour, before a line (Home's Coming up). */
export function KindDot({ kind, hollow = false, label }: KindDotProps) {
  return (
    <span
      className={styles["kindDot"]}
      data-kind={kind}
      data-hollow={hollow || undefined}
      {...(label === undefined ? { "aria-hidden": true } : { role: "img", "aria-label": label })}
      title={label}
    />
  );
}

/** A kind as a label (the event sheet's first line). */
export function KindChip({ kind, children }: { kind: CalendarKind; children: ReactNode }) {
  return (
    <span className={styles["kindChip"]} data-kind={kind}>
      <Icon name={KIND_ICONS[kind]} />
      {children}
    </span>
  );
}

/** The same chip as a button that opens a menu (change the kind). */
export function KindChipButton({
  kind,
  children,
  ...rest
}: { kind: CalendarKind; children: ReactNode } & Omit<
  ComponentPropsWithRef<"button">,
  "children" | "className"
>) {
  return (
    <button type="button" {...rest} className={styles["kindChip"]} data-kind={kind}>
      <Icon name={KIND_ICONS[kind]} />
      {children}
      <Icon name="chevron-down" />
    </button>
  );
}

export interface ViewTabsProps<T extends string> {
  /** Names the group ("View"). */
  label: string;
  options: readonly { value: T; label: string; shortcut: ShortcutId }[];
  value: T;
  onChange: (value: T) => void;
}

/** Agenda, Week, Month: each with its key (small and quiet), one pressed; sized like Segmented. */
export function ViewTabs<T extends string>({ label, options, value, onChange }: ViewTabsProps<T>) {
  return (
    <div className={styles["views"]} role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={styles["view"]}
          aria-pressed={option.value === value}
          onClick={() => {
            onChange(option.value);
          }}
        >
          {option.label}
          <KbdHint shortcut={option.shortcut} size="sm" />
        </button>
      ))}
    </div>
  );
}

export interface CalendarHeaderProps {
  /** "Calendar". */
  title: string;
  /** "Tuesday 27 October – Monday 2 November · Lisbon time". */
  period: string;
  /** The view switch ({@link ViewTabs}). */
  views: ReactNode;
  /** The toolbar: {@link OverflowActions}, so it never wraps. */
  actions: ReactNode;
}

/**
 * The calendar's header: title and period, the views, then the toolbar. On a
 * phone the views take a row of their own under the title and the toolbar.
 */
export function CalendarHeader({ title, period, views, actions }: CalendarHeaderProps) {
  return (
    <header className={styles["header"]}>
      <div className={styles["headTitle"]}>
        <Heading level={1} size="page">
          {title}
        </Heading>
        <span className={styles["period"]}>{period}</span>
      </div>
      <div className={styles["headViews"]}>{views}</div>
      <div className={styles["headActions"]}>{actions}</div>
    </header>
  );
}

/* Agenda */

export interface AgendaProps {
  children: ReactNode;
}

export function Agenda({ children }: AgendaProps) {
  return <div className={styles["agenda"]}>{children}</div>;
}

export interface AgendaDayProps {
  /** Its id, so the phone's day strip can scroll to it. */
  id: string;
  name: string;
  sub: string;
  /** "Today", on today's heading. */
  today?: string | undefined;
  children: ReactNode;
}

/** One day of the agenda: a sticky heading and its rows. */
export function AgendaDay({ id, name, sub, today, children }: AgendaDayProps) {
  return (
    <section className={styles["day"]} id={id} aria-labelledby={`${id}-name`}>
      <h2 className={styles["dayHead"]}>
        <span id={`${id}-name`} className={styles["dayName"]}>
          {name}
        </span>
        <span className={styles["daySub"]}>{sub}</span>
        {today === undefined ? null : <span className={styles["todayMark"]}>{today}</span>}
      </h2>
      {children}
    </section>
  );
}

export interface AgendaRowProps {
  kind: CalendarKind;
  /** "09:30 – 10:00", "All day", "Overdue · 2 days". */
  when: string;
  /** Drawn in the danger colour (an overdue task). */
  overdue?: boolean | undefined;
  /**
   * Whether `when` repeats in the second line on a phone (the default). Off
   * when it only says the kind again, as a task's plain "Due" does.
   */
  whenInline?: boolean | undefined;
  title: string;
  /** The kind's name, in its colour, before the rest of the line. */
  kindLabel: string;
  /** Place, calendar, amount: the rest of the second line. */
  meta: string;
  /** The message it was read from: "From email", and the sender where it is known. */
  from?: { label: string; sender?: string | undefined } | undefined;
  selected: boolean;
  onSelect: () => void;
}

/**
 * An event on a day: when, kind icon, title and a second line. On a phone
 * the time moves into the second line and the sender into a mail icon.
 */
export function AgendaRow({
  kind,
  when,
  overdue = false,
  whenInline = true,
  title,
  kindLabel,
  meta,
  from,
  selected,
  onSelect,
}: AgendaRowProps) {
  const tone = overdue ? "overdue" : undefined;
  return (
    <button
      type="button"
      className={styles["row"]}
      data-kind={kind}
      aria-current={selected || undefined}
      onClick={onSelect}
    >
      <span className={styles["when"]} data-tone={tone}>
        {when}
      </span>
      <KindIcon kind={kind} />
      <span className={styles["what"]}>
        <span className={styles["title"]}>{title}</span>
        <span className={styles["meta"]}>
          <span className={styles["metaKind"]}>{kindLabel}</span>
          <span> · </span>
          {whenInline ? (
            <span className={styles["whenInline"]} data-tone={tone}>
              {when}
              {" · "}
            </span>
          ) : null}
          {meta}
        </span>
      </span>
      {from === undefined ? null : (
        <span className={styles["from"]}>
          <Icon name="mail" />
          <VisuallyHidden>{from.label}</VisuallyHidden>
          <span className={styles["fromText"]} aria-hidden="true">
            {from.sender ?? from.label}
          </span>
        </span>
      )}
    </button>
  );
}

export interface DayStripDay {
  key: string;
  weekday: string;
  num: string;
  /** Full name, for assistive technology ("Tuesday 27 October"). */
  label: string;
  today: boolean;
  /** How many dots: what the day holds, at most three. */
  dots: number;
  onSelect: () => void;
}

/** The week's days as one row of touch targets, on phones only. */
export function DayStrip({ label, days }: { label: string; days: readonly DayStripDay[] }) {
  return (
    <div className={styles["strip"]} role="group" aria-label={label}>
      {days.map((day) => (
        <button
          key={day.key}
          type="button"
          className={styles["stripDay"]}
          aria-current={day.today ? "date" : undefined}
          aria-label={day.label}
          onClick={day.onSelect}
        >
          <span className={styles["stripWeekday"]} aria-hidden="true">
            {day.weekday}
          </span>
          <span className={styles["stripNum"]} aria-hidden="true">
            {day.num}
          </span>
          <span className={styles["stripDots"]} aria-hidden="true">
            {Array.from({ length: Math.min(day.dots, 3) }, (_, index) => (
              <span key={index} className={styles["stripDot"]} />
            ))}
          </span>
        </button>
      ))}
    </div>
  );
}

/* Week */

export interface WeekDayHead {
  key: string;
  weekday: string;
  num: string;
  label: string;
  today: boolean;
}

export interface WeekPill {
  id: string;
  kind: CalendarKind;
  title: string;
  /** The first and the last (exclusive) of the week's 7 columns it covers, 1-based. */
  from: number;
  to: number;
  selected: boolean;
  /** A task due: outlined, not filled. */
  dashed?: boolean | undefined;
  onSelect: () => void;
}

export interface WeekBlock {
  id: string;
  kind: CalendarKind;
  title: string;
  when: string;
  /** Minutes from the first hour shown. */
  start: number;
  length: number;
  /** Side by side with the events it overlaps: its place of that many. */
  column: number;
  columns: number;
  /** Set when read from a message: its mail mark's label. */
  fromLabel?: string | undefined;
  /** Time held for a task: hatched, not coloured. */
  held?: boolean | undefined;
  selected: boolean;
  onSelect: () => void;
}

export interface WeekGridProps {
  label: string;
  days: readonly WeekDayHead[];
  /** The Due lane above All day, only when there are tasks to show. */
  due?: { label: string; pills: readonly WeekPill[] } | undefined;
  allDay: { label: string; pills: readonly WeekPill[] };
  /** The hours drawn, `from` to `to` (exclusive), and how an hour is written. */
  hours: { from: number; to: number; format: (hour: number) => string };
  /** One list of blocks per day. */
  blocks: readonly (readonly WeekBlock[])[];
  /** The line at the current time: the day's index and the minutes from the first hour. */
  now?: { day: number; minute: number } | undefined;
}

function Lane({ label, pills }: { label: string; pills: readonly WeekPill[] }) {
  return (
    <div className={styles["lane"]}>
      <div className={styles["laneLabel"]}>{label}</div>
      <div className={styles["laneCells"]}>
        {pills.map((pill) => (
          <button
            key={pill.id}
            type="button"
            className={styles["pill"]}
            data-kind={pill.kind}
            data-dashed={pill.dashed === true ? "" : undefined}
            aria-current={pill.selected || undefined}
            style={{ gridColumn: `${String(pill.from)} / ${String(pill.to)}` }}
            title={pill.title}
            onClick={pill.onSelect}
          >
            <Icon name={KIND_ICONS[pill.kind]} />
            <span>{pill.title}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

/**
 * The week as a time grid: day headings, an optional Due lane, an All day
 * lane, and a column per day with events placed by the minute, side by side
 * when they overlap. The grid scrolls inside itself on narrow screens.
 */
export function WeekGrid({ label, days, due, allDay, hours, blocks, now }: WeekGridProps) {
  const hourList = Array.from({ length: hours.to - hours.from }, (_, index) => hours.from + index);
  return (
    <div className={styles["weekScroll"]} role="region" aria-label={label}>
      <div className={styles["weekInner"]}>
        <div className={styles["weekHead"]}>
          <div />
          <div className={styles["weekDays"]}>
            {days.map((day) => (
              <div
                key={day.key}
                className={styles["weekDay"]}
                aria-current={day.today ? "date" : undefined}
              >
                <span aria-hidden="true">{day.weekday}</span>
                <b className={styles["weekNum"]} aria-hidden="true">
                  {day.num}
                </b>
                <VisuallyHidden>{day.label}</VisuallyHidden>
              </div>
            ))}
          </div>
        </div>
        {due === undefined ? null : <Lane label={due.label} pills={due.pills} />}
        <Lane label={allDay.label} pills={allDay.pills} />
        <div className={styles["hours"]}>
          <div className={styles["hourLabels"]} aria-hidden="true">
            {hourList.map((hour) => (
              <div key={hour} className={styles["hourLabel"]}>
                {hours.format(hour)}
              </div>
            ))}
          </div>
          <div className={styles["columns"]}>
            {days.map((day, index) => (
              <div
                key={day.key}
                className={styles["column"]}
                style={{ height: `calc(${String(hourList.length)} * var(--calendar-hour))` }}
              >
                {now?.day === index ? (
                  <div className={styles["now"]} style={place({ "--now": now.minute })} />
                ) : null}
                {(blocks[index] ?? []).map((block) => (
                  <button
                    key={block.id}
                    type="button"
                    className={styles["block"]}
                    data-kind={block.kind}
                    data-held={block.held === true ? "" : undefined}
                    aria-current={block.selected || undefined}
                    title={`${block.title}, ${block.when}`}
                    style={place({
                      "--start": block.start,
                      "--len": block.length,
                      "--column": block.column,
                      "--columns": block.columns,
                    })}
                    onClick={block.onSelect}
                  >
                    <span className={styles["blockTitle"]}>
                      {block.fromLabel === undefined ? null : (
                        <>
                          <Icon name="mail" className={styles["blockMail"]} />
                          <VisuallyHidden>{block.fromLabel}</VisuallyHidden>
                        </>
                      )}
                      {block.title}
                    </span>
                    {block.length >= 50 ? (
                      <span className={styles["blockWhen"]}>{block.when}</span>
                    ) : null}
                  </button>
                ))}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

/* Month */

export interface MonthEvent {
  id: string;
  kind: CalendarKind;
  title: string;
  selected: boolean;
  onSelect: () => void;
}

export interface MonthDay {
  key: string;
  num: string;
  label: string;
  today: boolean;
  /** A day of the neighbouring month. */
  outside: boolean;
  events: readonly MonthEvent[];
  /** "+2 more": opens the day. */
  more?: string | undefined;
  onOpen: () => void;
}

export interface MonthGridProps {
  label: string;
  weekdays: readonly string[];
  weeks: readonly (readonly MonthDay[])[];
}

/** A month: each day's first events and "+N more"; on phones, one dot per event. */
export function MonthGrid({ label, weekdays, weeks }: MonthGridProps) {
  return (
    <div className={styles["month"]} role="group" aria-label={label}>
      <div className={styles["monthWeekdays"]} aria-hidden="true">
        {weekdays.map((weekday) => (
          <div key={weekday} className={styles["monthWeekday"]}>
            {weekday}
          </div>
        ))}
      </div>
      {weeks.map((week) => (
        <div key={week[0]?.key} className={styles["monthWeek"]}>
          {week.map((day) => (
            <div
              key={day.key}
              className={styles["monthCell"]}
              data-outside={day.outside ? "" : undefined}
              aria-current={day.today ? "date" : undefined}
            >
              <button
                type="button"
                className={styles["monthNum"]}
                aria-label={day.label}
                onClick={day.onOpen}
              >
                {day.num}
              </button>
              <div className={styles["monthEvents"]}>
                {day.events.map((event) => (
                  <button
                    key={event.id}
                    type="button"
                    className={styles["monthEvent"]}
                    data-kind={event.kind}
                    aria-label={event.title}
                    aria-current={event.selected || undefined}
                    onClick={event.onSelect}
                  >
                    <span className={styles["monthEventTitle"]}>{event.title}</span>
                  </button>
                ))}
                {day.more === undefined ? null : (
                  <button type="button" className={styles["monthMore"]} onClick={day.onOpen}>
                    {day.more}
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

/* The event sheet */

export interface EventSheetProps {
  /** Names the sheet (the event's title). */
  label: string;
  children: ReactNode;
}

/**
 * The event's sheet: the calendar's right-hand pane from the large
 * breakpoint, the whole screen below it.
 */
export function EventSheet({ label, children }: EventSheetProps) {
  return (
    <aside className={styles["sheet"]} aria-label={label}>
      {children}
    </aside>
  );
}

/** The sheet's first row: the kind and the close button. */
export function SheetTop({ children }: { children: ReactNode }) {
  return <div className={styles["sheetTop"]}>{children}</div>;
}

/** The event's title. */
export function SheetTitle({ children }: { children: ReactNode }) {
  return (
    <Heading level={2} size="title">
      {children}
    </Heading>
  );
}

/** The sheet's buttons: wrapping, full-size touch targets on phones. */
export function SheetActions({ children }: { children: ReactNode }) {
  return <div className={styles["actions"]}>{children}</div>;
}

/** Facts about the event: icon and text per row. */
export function SheetFacts({ children }: { children: ReactNode }) {
  return <div className={styles["facts"]}>{children}</div>;
}

export function SheetFact({
  icon,
  main,
  children,
}: {
  icon: IconName;
  /** The bold first line. */
  main?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <>
      <span className={styles["factIcon"]}>
        <Icon name={icon} />
      </span>
      <div className={styles["fact"]}>
        {main === undefined ? null : <strong className={styles["factMain"]}>{main}</strong>}
        {children === undefined ? null : <span>{children}</span>}
      </div>
    </>
  );
}

export interface SourceLinkProps extends Omit<ComponentPropsWithRef<"a">, "children"> {
  /** "From email". */
  label: string;
  /** The message's subject. */
  subject: string;
  /** Who sent it and when, then the "Open email" call. */
  meta: ReactNode;
  shortcut?: ShortcutId | undefined;
}

/** The message an event was read from; opens it in the reader. */
export function SourceLink({
  label,
  subject,
  meta,
  shortcut,
  className,
  ...rest
}: SourceLinkProps) {
  return (
    <a {...rest} className={cx(styles["source"], className)}>
      <span className={styles["caps"]}>{label}</span>
      <span className={styles["sourceTitle"]}>
        <Icon name="mail" />
        {subject}
      </span>
      <span className={styles["sourceMeta"]}>
        {meta}
        {shortcut === undefined ? null : <KbdHint shortcut={shortcut} />}
      </span>
    </a>
  );
}

/** A labelled block of the sheet ("How Scout knew"). */
export function SheetSection({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles["section"]}>
      <span className={styles["caps"]}>{label}</span>
      {children}
    </div>
  );
}

/** The sentence Scout read a date from, as written. */
export function SheetQuote({ children }: { children: ReactNode }) {
  return <blockquote className={styles["quote"]}>{children}</blockquote>;
}

/** A line of the sheet that says something about the event's state ("we kept theirs"). */
export function SheetNotice({ children }: { children: ReactNode }) {
  return (
    <p className={styles["notice"]} role="status">
      {children}
    </p>
  );
}

/* What Scout found, in the reader */

export interface FoundStripProps {
  /** "Scout noticed". */
  label: string;
  children: ReactNode;
}

/** The strip under a message's header. */
export function FoundStrip({ label, children }: FoundStripProps) {
  return (
    <section className={styles["found"]} aria-label={label}>
      <div className={styles["foundLabel"]}>
        <Visor size="xs" />
        {label}
      </div>
      {children}
    </section>
  );
}

export interface FoundItemProps {
  kind: CalendarKind;
  title: string;
  /** Kind, when and where, in one line. */
  meta: string;
  /** The sentence it was read from. */
  quote?: string | undefined;
  /** Where it is now ("Added to your calendar"). */
  status?: ReactNode;
  /** Buttons: Undo, Add to my calendar, Add. */
  actions?: ReactNode;
}

export function FoundItem({ kind, title, meta, quote, status, actions }: FoundItemProps) {
  return (
    <div className={styles["foundItem"]}>
      <KindIcon kind={kind} />
      <div className={styles["foundText"]}>
        <span className={styles["foundTitle"]}>{title}</span>
        <span className={styles["foundMeta"]}>{meta}</span>
        {quote === undefined ? null : <SheetQuote>{quote}</SheetQuote>}
      </div>
      <div className={styles["foundActions"]}>
        {status === undefined ? null : <span className={styles["foundStatus"]}>{status}</span>}
        {actions}
      </div>
    </div>
  );
}

/** "Scout is reading this…", with a bar that sweeps while it does. */
export function FoundReading({ children }: { children: ReactNode }) {
  return (
    <div className={styles["reading"]} role="status">
      <span>{children}</span>
      <span className={styles["readingBar"]} aria-hidden="true" />
    </div>
  );
}

/* Settings */

/** A calendar's colour, as the provider gave it (any `#rrggbb`, so not a token). */
export function CalendarDot({ color }: { color?: string | null | undefined }) {
  return (
    <span
      className={styles["dot"]}
      style={color === undefined || color === null ? undefined : { background: color }}
      aria-hidden="true"
    />
  );
}

export interface CalendarColourProps {
  /** Names the colour ("Colour of Work"). */
  label: string;
  /** `#rrggbb`, as the provider gave it. */
  color?: string | null | undefined;
  /** Called with the colour picked once the picker closes. */
  onChange: (color: string) => void;
}

/** A calendar's colour as a dot that opens the platform's colour picker. */
export function CalendarColour({ label, color, onChange }: CalendarColourProps) {
  const [picked, setPicked] = useState(color ?? "#808080");
  return (
    <label className={styles["colour"]}>
      <CalendarDot color={picked} />
      <input
        className={styles["colourInput"]}
        type="color"
        aria-label={label}
        value={picked}
        onChange={(event) => {
          setPicked(event.target.value);
        }}
        onBlur={() => {
          if (picked !== color) {
            onChange(picked);
          }
        }}
      />
    </label>
  );
}
