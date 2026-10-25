import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import type { CalendarKind } from "~/lib/ui";

import {
  useCalendars,
  useEventActions,
  useEvents,
  useScheduleSettings,
  useSourceMessage,
} from "./api/queries";
import type { EventView } from "./api/types";
import { useClock } from "./clock";
import { emailPath } from "./components/Calendar/links";
import {
  COMING_DAYS,
  comingUp,
  dayFacts,
  joinUrl,
  laterToday,
  minutesUntil,
  nextUp,
  offersAdd,
  pushTarget,
  type Coming,
  type WallClock,
} from "./day";
import { useTaskRows } from "./due";
import { dueWhen, eventKey, metaLine, whenLabel, type T } from "./labels";
import { addDays, clock as clockText, dayNumber, dayOf, dayShort, minutesOf, nowIn } from "./model";

/** An event as one of Home's rows (campbooks-cloud#359). */
export interface DayEvent {
  /** The event or occurrence (`eventKey`). */
  id: string;
  kind: CalendarKind;
  /** Before the title: the kind's name, then when ("In 20 min", "14:00", "All day"). */
  eyebrow: { kind: string; reason: string };
  title: string;
  /** The second line: the place (the host of a web place), else the calendar's name. */
  meta: string;
  /** At the row's end: "10:00 – 10:30", "All day". */
  when: string;
  /** Opens the event's web address; only when its place is one (`https://`). */
  join?: (() => void) | undefined;
  /** The Calendar on the event's day, the event open. */
  openInCalendar: () => void;
  /** The email it was read from, in the reader; known once the row is selected. */
  openEmail?: (() => void) | undefined;
}

/** One line of Coming up. */
export interface DayLine {
  id: string;
  /** Its day, short: "Tue 27". */
  day: string;
  kind: CalendarKind;
  /** Only the member sees it (on no provider calendar): a ring, and this sentence for those who cannot see it. */
  onlyYou?: string | undefined;
  title: string;
  /** At the end: its time, "Due" for a task. */
  meta?: string | undefined;
  /** "Add to my calendar", with the toast's Undo. */
  add?: { label: string; name: string; run: () => void } | undefined;
}

/** The member's day, for Home. Empty until the calendar answers, and when it fails. */
interface HomeDay {
  nextUp?: DayEvent | undefined;
  /** The rest of today: at the end of Today, after its items. */
  today: DayEvent[];
  /** Tomorrow to a week after it, soonest first. */
  coming: DayLine[];
  /** For the hero: the next event of today and how many more there are. */
  facts?: { title: string; time: string; more: number } | undefined;
  /** The Calendar place. */
  calendarHref: string;
  openCalendar: () => void;
}

interface HomeDayOptions {
  /** Pins the clock (stories and tests); otherwise it ticks once a minute. */
  now?: Date | undefined;
  /** The event row Home has selected (`DayEvent.id`): its email is looked up. */
  selected?: string | undefined;
}

/** "In 20 min", "In 1 h 10 min", "Now", "Started 5 min ago". */
function countdown(t: T, minutes: number): string {
  if (minutes < 0) {
    return t("home.started", { minutes: -minutes });
  }
  if (minutes === 0) {
    return t("home.now");
  }
  if (minutes < 60) {
    return t("home.inMinutes", { minutes });
  }
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0
    ? t("home.inHours", { hours })
    : t("home.inHoursMinutes", { hours, minutes: rest });
}

/**
 * What Home shows of the member's calendar: Next up, the rest of today and
 * Coming up, in words, with what each row can do. The calendar's own rules:
 * the member's zone, their shown calendars only (the server leaves out a
 * calendar they turned off), the tasks of the Due layer.
 */
export function useHomeDay(workspaceId: string, { now, selected }: HomeDayOptions = {}): HomeDay {
  const { t: typed, i18n } = useTranslation("schedule");
  const t: T = typed;
  const locale = i18n.language;
  const navigate = useNavigate();
  const settings = useScheduleSettings(workspaceId);
  const calendars = useCalendars(workspaceId);
  const zone = settings.data?.effective_time_zone ?? "UTC";
  const clock: WallClock = nowIn(zone, useClock(now));
  // Today and the week after it, in one read.
  const events = useEvents(
    workspaceId,
    { from: clock.day, to: addDays(clock.day, COMING_DAYS + 1) },
    { enabled: settings.data !== undefined },
  );
  const taskRows = useTaskRows(workspaceId, clock.day, zone);
  const actions = useEventActions(workspaceId);

  const calendarList = calendars.data ?? [];
  // Nothing from a calendar that has not answered, or failed: Home as without one.
  const list = events.isError || settings.isError ? [] : (events.data?.events ?? []);
  const next = nextUp(list, clock);
  const rest = laterToday(list, clock, next);
  const chosen = [...(next === undefined ? [] : [next]), ...rest].find(
    (event) => eventKey(event) === selected,
  );
  const source = useSourceMessage(workspaceId, chosen);

  const calendarHref = `/w/${workspaceId}/schedule`;
  const openCalendar = () => {
    void navigate({ to: "/w/$workspaceId/schedule", params: { workspaceId } });
  };

  const row = (event: EventView, reason: string): DayEvent => {
    const key = eventKey(event);
    const url = joinUrl(event);
    const day = dayOf(event.starts);
    const sourceData = chosen !== undefined && eventKey(chosen) === key ? source.data : undefined;
    return {
      id: key,
      kind: event.kind,
      eyebrow: { kind: t(`kind.${event.kind}`), reason },
      title: event.title,
      meta: url === undefined ? metaLine(event, calendarList) : url.host,
      when: whenLabel(t, locale, event),
      join:
        url === undefined
          ? undefined
          : () => {
              globalThis.open(url.href, "_blank", "noopener,noreferrer");
            },
      openInCalendar: () => {
        void navigate({
          to: "/w/$workspaceId/schedule",
          params: { workspaceId },
          search: { ...(day === clock.day ? {} : { date: day }), event: key },
        });
      },
      openEmail:
        sourceData === undefined
          ? undefined
          : () => {
              void navigate({
                to: emailPath(workspaceId, sourceData.stream, sourceData.conversationId),
              });
            },
    };
  };

  const today = rest.map((event) =>
    row(event, event.all_day ? t("when.allDay") : clockText(locale, minutesOf(event.starts))),
  );

  const target = pushTarget(calendarList, settings.data?.last_push_calendar);
  const line = (coming: Coming): DayLine => {
    const day = `${dayShort(locale, coming.day)} ${dayNumber(coming.day)}`;
    if (coming.type === "task") {
      return {
        id: coming.row.key,
        day,
        kind: "due",
        title: coming.row.task.title,
        meta: dueWhen(t, locale, coming.row, clock.day),
      };
    }
    const { event } = coming;
    const add =
      target !== undefined && offersAdd(event, calendarList)
        ? {
            label: t("sheet.addToMine"),
            name: t("home.addNamed", { title: event.title }),
            run: () => {
              actions.addTo.mutate({ event, calendar: target });
            },
          }
        : undefined;
    return {
      id: eventKey(event),
      day,
      kind: event.kind,
      onlyYou: event.on_provider ? undefined : t("sheet.onlyYou"),
      title: event.title,
      meta: event.all_day ? undefined : clockText(locale, minutesOf(event.starts)),
      add,
    };
  };
  // Without an event this week Home stays as it was: no Coming up for tasks
  // alone. With one, the tasks due come with the events, all at once.
  const coming = list.length === 0 ? [] : comingUp(list, taskRows, clock);

  const facts = dayFacts([...(next === undefined ? [] : [next]), ...rest]);
  return {
    nextUp: next === undefined ? undefined : row(next, countdown(t, minutesUntil(next, clock))),
    today,
    coming: coming.map((each) => line(each)),
    facts:
      facts === undefined
        ? undefined
        : {
            title: facts.next.title,
            time: clockText(locale, minutesOf(facts.next.starts)),
            more: facts.more,
          },
    calendarHref,
    openCalendar,
  };
}
