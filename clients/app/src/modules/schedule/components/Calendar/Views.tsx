import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import {
  Agenda,
  AgendaDay,
  AgendaRow,
  DayStrip,
  MonthGrid,
  Text,
  WeekGrid,
  type MonthDay,
  type WeekBlock,
  type WeekPill,
} from "~/lib/ui";

import type { CalendarView, EventView } from "../../api/types";
import type { DueItem } from "../../due";
import {
  dueKeyOf,
  dueStandsOn,
  dueTitle,
  dueWhen,
  eventKey,
  metaLine,
  overdueDays,
  whenLabel,
  type T,
} from "../../labels";
import {
  agendaOn,
  blocksOn,
  dayLong,
  dayName,
  dayNumber,
  daySub,
  dayShort,
  daysBetween,
  hourLabel,
  hoursFor,
  laneOf,
  MONTH_SHOWN,
  monthStart,
  startMinutes,
  dueMinutes,
  type Ymd,
} from "../../model";

export interface ViewProps {
  /** The days the view covers, in order. */
  days: readonly Ymd[];
  events: readonly EventView[];
  due: readonly DueItem[];
  calendars: readonly CalendarView[];
  today: { day: Ymd; minute: number };
  /** The key of the event or Due row whose sheet is open. */
  selected: string | undefined;
  onSelect: (key: string) => void;
  /** Go to a day in the agenda (a month cell's number). */
  onOpenDay: (day: Ymd) => void;
}

const typedT = (t: unknown) => t as T;

function useLabels() {
  const { t, i18n } = useTranslation("schedule");
  return { t: typedT(t), locale: i18n.language };
}

const fromEmail = (t: T, event: EventView) =>
  event.source?.type.startsWith("conversations.") === true ? { label: t("from.email") } : undefined;

/** One row of a day, with what orders it among the others. */
interface Row {
  key: string;
  /** Overdue first, then all-day, then tasks due that day, then the timed ones. */
  group: number;
  minute: number;
  node: ReactNode;
}

/** Days from today onwards, each with its events in time order; days with nothing are skipped. */
export function AgendaView(props: ViewProps) {
  const { t, locale } = useLabels();
  const { days, events, due, calendars, today, selected, onSelect } = props;
  const dueShown = due.filter((item) => item.held !== true);

  const sections = days
    .map((day) => {
      const rows: Row[] = [
        ...agendaOn(events, day).map((event): Row => {
          const key = eventKey(event);
          return {
            key,
            group: event.all_day ? 1 : 3,
            minute: startMinutes(event),
            node: (
              <AgendaRow
                key={key}
                kind={event.kind}
                when={whenLabel(t, locale, event)}
                title={event.title}
                kindLabel={t(`kind.${event.kind}`)}
                meta={metaLine(event, calendars)}
                from={fromEmail(t, event)}
                selected={selected === key}
                onSelect={() => {
                  onSelect(key);
                }}
              />
            ),
          };
        }),
        ...dueShown
          .filter((item) => dueStandsOn(item, today.day) === day)
          .map((item): Row => {
            const key = dueKeyOf(item);
            const late = overdueDays(item, today.day) > 0;
            return {
              key,
              group: late ? 0 : dueMinutes(item) === undefined ? 2 : 3,
              minute: dueMinutes(item) ?? 0,
              node: (
                <AgendaRow
                  key={key}
                  kind="due"
                  when={dueWhen(t, locale, item, today.day)}
                  overdue={late}
                  title={dueTitle(t, locale, item)}
                  kindLabel={t("kind.due")}
                  meta={t("due.meta")}
                  from={
                    item.source === undefined || item.source === null
                      ? undefined
                      : { label: t("from.email"), sender: item.source.sender }
                  }
                  selected={selected === key}
                  onSelect={() => {
                    onSelect(key);
                  }}
                />
              ),
            };
          }),
      ].toSorted((a, b) => a.group - b.group || a.minute - b.minute);
      return { day, rows };
    })
    .filter((section) => section.rows.length > 0);

  const strip = days.map((day) => ({
    key: day,
    weekday: dayShort(locale, day),
    num: dayNumber(day),
    label: dayLong(locale, day),
    today: day === today.day,
    dots: sections.find((section) => section.day === day)?.rows.length ?? 0,
    onSelect: () => {
      document.getElementById(`agenda-${day}`)?.scrollIntoView({ block: "start" });
    },
  }));

  return (
    <>
      <DayStrip label={t("agenda.week")} days={strip} />
      <Agenda>
        {sections.length === 0 ? (
          <Text tone="secondary">{t("agenda.empty")}</Text>
        ) : (
          sections.map((section) => (
            <AgendaDay
              key={section.day}
              id={`agenda-${section.day}`}
              name={dayName(locale, section.day)}
              sub={daySub(locale, section.day)}
              today={section.day === today.day ? t("agenda.today") : undefined}
            >
              {section.rows.map((row) => row.node)}
            </AgendaDay>
          ))
        )}
      </Agenda>
    </>
  );
}

/** The week as a time grid: a Due lane (only with tasks), All day, and a column per day. */
export function WeekView(props: ViewProps) {
  const { t, locale } = useLabels();
  const { days, events, due, today, selected, onSelect } = props;
  const dueShown = due.filter((item) => item.held !== true);
  const first = days[0] ?? today.day;

  const placed = days.map((day) => blocksOn(events, day));
  const hours = hoursFor(placed.flat());
  const blocks: WeekBlock[][] = placed.map((day) =>
    day.map(({ event, start, end, column, columns }) => {
      const key = eventKey(event);
      return {
        id: key,
        kind: event.kind,
        title: event.title,
        when: whenLabel(t, locale, event),
        start: start - hours.from * 60,
        length: end - start,
        column,
        columns,
        fromLabel: fromEmail(t, event)?.label,
        held: event.kind === "hold",
        selected: selected === key,
        onSelect: () => {
          onSelect(key);
        },
      };
    }),
  );
  const allDay: WeekPill[] = laneOf(events, days).map(({ event, from, to }) => {
    const key = eventKey(event);
    return {
      id: key,
      kind: event.kind,
      title: event.title,
      from,
      to,
      selected: selected === key,
      onSelect: () => {
        onSelect(key);
      },
    };
  });
  const dueLane: WeekPill[] = dueShown
    .filter((item) => {
      const day = dueStandsOn(item, today.day);
      return day >= first && day <= (days.at(-1) ?? first);
    })
    .map((item): WeekPill => {
      const column = daysBetween(first, dueStandsOn(item, today.day)) + 1;
      const key = dueKeyOf(item);
      return {
        id: key,
        kind: "due",
        title: dueTitle(t, locale, item),
        from: column,
        to: column + 1,
        dashed: true,
        selected: selected === key,
        onSelect: () => {
          onSelect(key);
        },
      };
    });
  const todayColumn = days.indexOf(today.day);
  return (
    <WeekGrid
      label={t("week.label")}
      days={days.map((day) => ({
        key: day,
        weekday: dayShort(locale, day),
        num: dayNumber(day),
        label: dayLong(locale, day),
        today: day === today.day,
      }))}
      due={dueLane.length === 0 ? undefined : { label: t("week.due"), pills: dueLane }}
      allDay={{ label: t("week.allDay"), pills: allDay }}
      hours={{ from: hours.from, to: hours.to, format: (hour) => hourLabel(locale, hour) }}
      blocks={blocks}
      now={
        todayColumn < 0 ? undefined : { day: todayColumn, minute: today.minute - hours.from * 60 }
      }
    />
  );
}

/** A month: each day's first events and "+N more". */
export function MonthView(props: ViewProps) {
  const { t, locale } = useLabels();
  const { days, events, today, selected, onSelect, onOpenDay } = props;
  const anchorMonth = monthStart(days[Math.floor(days.length / 2)] ?? today.day).slice(0, 7);
  const cells: MonthDay[] = days.map((day) => {
    const list = agendaOn(events, day);
    const shown = list.slice(0, MONTH_SHOWN);
    const rest = list.length - shown.length;
    return {
      key: day,
      num: dayNumber(day),
      label: dayLong(locale, day),
      today: day === today.day,
      outside: day.slice(0, 7) !== anchorMonth,
      events: shown.map((event) => {
        const key = eventKey(event);
        return {
          id: key,
          kind: event.kind,
          title: event.title,
          selected: selected === key,
          onSelect: () => {
            onSelect(key);
          },
        };
      }),
      more: rest > 0 ? t("month.more", { count: rest }) : undefined,
      onOpen: () => {
        onOpenDay(day);
      },
    };
  });
  const weeks = Array.from({ length: Math.ceil(cells.length / 7) }, (_, index) =>
    cells.slice(index * 7, index * 7 + 7),
  );
  const weekdays = days.slice(0, 7).map((day) => dayShort(locale, day));
  return <MonthGrid label={t("month.label")} weekdays={weekdays} weeks={weeks} />;
}
