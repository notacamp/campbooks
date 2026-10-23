import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../Button/Button";
import { describedBy, FieldFrame, useFieldIds } from "../Field/Field";
import { Icon } from "../Icon/Icon";
import { IconButton } from "../IconButton/IconButton";
import { Popover } from "../Popover/Popover";

import {
  addDays,
  addMonths,
  clampDay,
  dayToDate,
  endOfWeek,
  inRange,
  monthMatrix,
  parseDay,
  sameMonth,
  startOfWeek,
  todayOf,
  weekStart,
  type Day,
} from "./dates";
import styles from "./DatePicker.module.css";

export interface DatePickerProps {
  /** The field's name ("Due"); the grid's too when `inline`. */
  label: string;
  /** Keep the label for screen readers only. */
  hideLabel?: boolean | undefined;
  /** The chosen day ("2026-10-26"), or none. */
  value: string | undefined;
  /** A day was picked (or cleared). Never called while browsing months. */
  onChange: (day: string | undefined) => void;
  /** The earliest day that can be picked. */
  min?: string | undefined;
  /** The latest day that can be picked. */
  max?: string | undefined;
  /** What the field says with no day ("Choose a date" by default). */
  placeholder?: string | undefined;
  id?: string | undefined;
  /** Offer Clear, which picks no day. */
  clearable?: boolean | undefined;
  /** The month grid itself, in place, with no field to open it. */
  inline?: boolean | undefined;
  /** Help under the field. */
  hint?: ReactNode | undefined;
  /** What is wrong with the day. */
  error?: ReactNode | undefined;
  /** Pins today (stories and tests). */
  now?: Date | undefined;
}

/** "Mon 26 Oct", with the year when it is not this year's. */
function shortDay(day: Day, locale: string, thisYear: number): string {
  const at = dayToDate(day);
  return new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
    ...(at.getUTCFullYear() === thisYear ? {} : { year: "numeric" }),
    timeZone: "UTC",
  }).format(at);
}

/**
 * A date field: a field-styled button showing the day in the member's
 * language that opens a month grid beside it, or (`inline`) the grid alone.
 * Keys in the grid: arrows move a day or a week, Page Up/Down a month (with
 * Shift a year), Home/End the week's start and end, Enter or Space picks,
 * Escape closes and gives the focus back to the field.
 */
export function DatePicker({
  label,
  hideLabel,
  value,
  onChange,
  min,
  max,
  placeholder,
  id,
  clearable = false,
  inline = false,
  hint,
  error,
  now,
}: DatePickerProps) {
  const { t, i18n } = useTranslation("ui");
  const ids = useFieldIds(id);
  const valueId = useId();
  const chosen = value !== undefined && parseDay(value) !== undefined ? value : undefined;
  const today = todayOf(now ?? new Date());

  if (inline) {
    return (
      <div className={styles["inline"]} role="group" aria-label={label}>
        <MonthCalendar
          value={chosen}
          today={today}
          min={min}
          max={max}
          clearable={clearable}
          locale={i18n.language}
          onPick={onChange}
        />
      </div>
    );
  }

  return (
    <FieldFrame label={label} hideLabel={hideLabel} hint={hint} error={error} ids={ids}>
      <Popover
        label={t("datePicker.choose")}
        placement="bottom-start"
        renderTrigger={(trigger) => (
          <button
            {...trigger}
            id={ids.controlId}
            type="button"
            className={styles["trigger"]}
            aria-labelledby={`${ids.labelId} ${valueId}`}
            aria-describedby={describedBy(hint, error, ids)}
            data-invalid={error === undefined ? undefined : ""}
          >
            <Icon name="schedule" />
            <span
              id={valueId}
              className={styles["value"]}
              data-empty={chosen === undefined || undefined}
            >
              {chosen === undefined
                ? (placeholder ?? t("datePicker.choose"))
                : shortDay(chosen, i18n.language, Number(today.slice(0, 4)))}
            </span>
          </button>
        )}
      >
        {(close) => (
          <MonthCalendar
            value={chosen}
            today={today}
            min={min}
            max={max}
            clearable={clearable}
            locale={i18n.language}
            onPick={(day) => {
              close();
              document.getElementById(ids.controlId)?.focus();
              onChange(day);
            }}
          />
        )}
      </Popover>
    </FieldFrame>
  );
}

interface MonthCalendarProps {
  value: Day | undefined;
  today: Day;
  min: Day | undefined;
  max: Day | undefined;
  clearable: boolean;
  locale: string;
  onPick: (day: Day | undefined) => void;
}

/** The month grid: month and year, previous and next, the days, Today and Clear. */
function MonthCalendar({ value, today, min, max, clearable, locale, onPick }: MonthCalendarProps) {
  const { t } = useTranslation("ui");
  const headingId = useId();
  const grid = useRef<HTMLDivElement>(null);
  const firstDay = weekStart(locale);
  // The day the keyboard is on; its month is the one shown.
  const [cursor, setCursor] = useState(() => clampDay(value ?? today, min, max));
  // A day chosen from outside (a chip beside an inline grid) shows its month.
  const [seen, setSeen] = useState(value);
  if (seen !== value) {
    setSeen(value);
    if (value !== undefined) {
      setCursor(clampDay(value, min, max));
    }
  }
  // Focus follows the cursor once the keys have moved it (never on first show).
  const moved = useRef(false);
  useEffect(() => {
    if (moved.current) {
      grid.current?.querySelector<HTMLElement>(`[data-day="${cursor}"]`)?.focus();
    }
  }, [cursor]);

  const go = (day: Day, focus: boolean) => {
    moved.current = focus;
    setCursor(clampDay(day, min, max));
  };
  const weeks = monthMatrix(cursor, firstDay);
  const at = (day: Day) => dayToDate(day);
  const fullDay = new Intl.DateTimeFormat(locale, {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  });
  const monthName = new Intl.DateTimeFormat(locale, {
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(at(cursor));
  const narrow = new Intl.DateTimeFormat(locale, { weekday: "narrow", timeZone: "UTC" });
  const long = new Intl.DateTimeFormat(locale, { weekday: "long", timeZone: "UTC" });
  const heads = weeks[0] ?? [];

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const moves: Record<string, () => Day> = {
      ArrowLeft: () => addDays(cursor, -1),
      ArrowRight: () => addDays(cursor, 1),
      ArrowUp: () => addDays(cursor, -7),
      ArrowDown: () => addDays(cursor, 7),
      PageUp: () => addMonths(cursor, event.shiftKey ? -12 : -1),
      PageDown: () => addMonths(cursor, event.shiftKey ? 12 : 1),
      Home: () => startOfWeek(cursor, firstDay),
      End: () => endOfWeek(cursor, firstDay),
    };
    const move = moves[event.key];
    if (move === undefined) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    go(move(), true);
  };

  const title = monthName.charAt(0).toLocaleUpperCase(locale) + monthName.slice(1);
  return (
    <div className={styles["calendar"]}>
      <div className={styles["head"]}>
        <IconButton
          label={t("datePicker.previous")}
          icon="chevron-left"
          disabled={min !== undefined && addDays(`${cursor.slice(0, 8)}01`, -1) < min}
          onClick={() => {
            go(addMonths(cursor, -1), false);
          }}
        />
        <p id={headingId} className={styles["month"]} aria-live="polite">
          {title}
        </p>
        <IconButton
          label={t("datePicker.next")}
          icon="chevron-right"
          disabled={max !== undefined && addMonths(`${cursor.slice(0, 8)}01`, 1) > max}
          onClick={() => {
            go(addMonths(cursor, 1), false);
          }}
        />
      </div>
      <div ref={grid} role="grid" className={styles["grid"]} aria-labelledby={headingId}>
        <div role="row" className={styles["week"]}>
          {heads.map((day) => (
            <div
              key={day}
              role="columnheader"
              className={styles["weekday"]}
              aria-label={long.format(at(day))}
            >
              {narrow.format(at(day))}
            </div>
          ))}
        </div>
        {weeks.map((week) => (
          <div key={week[0]} role="row" className={styles["week"]}>
            {week.map((day) => {
              const selected = day === value;
              return (
                <div key={day} role="gridcell" className={styles["cell"]} aria-selected={selected}>
                  <button
                    type="button"
                    className={styles["day"]}
                    data-day={day}
                    data-outside={sameMonth(day, cursor) ? undefined : ""}
                    data-selected={selected || undefined}
                    data-autofocus={day === cursor ? "" : undefined}
                    tabIndex={day === cursor ? 0 : -1}
                    disabled={!inRange(day, min, max)}
                    aria-current={day === today ? "date" : undefined}
                    aria-label={fullDay.format(at(day))}
                    onKeyDown={onKeyDown}
                    onClick={() => {
                      onPick(day);
                    }}
                  >
                    {Number(day.slice(8))}
                  </button>
                </div>
              );
            })}
          </div>
        ))}
      </div>
      <div className={styles["foot"]}>
        <Button
          variant="ghost"
          disabled={!inRange(today, min, max)}
          onClick={() => {
            onPick(today);
          }}
        >
          {t("datePicker.today")}
        </Button>
        {clearable ? (
          <Button
            variant="ghost"
            onClick={() => {
              onPick(undefined);
            }}
          >
            {t("datePicker.clear")}
          </Button>
        ) : null}
      </div>
    </div>
  );
}
