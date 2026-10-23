import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  CalendarHeader,
  OverflowActions,
  ReaderPane,
  SkeletonList,
  ViewTabs,
} from "~/lib/ui";

import { useCalendars, useEventActions, useEvents, useScheduleSettings } from "../../api/queries";
import type { NewEvent } from "../../api/types";
import { useDueActions, useDueItems } from "../../due";
import { dueKeyOf, eventKey, type T } from "../../labels";
import {
  addDays,
  cityOf,
  daySub,
  daysOf,
  monthTitle,
  nowIn,
  shift,
  VIEWS,
  windowOf,
  type View,
  type Ymd,
} from "../../model";
import { DuePane } from "./DuePane";
import { EventDialog, type EventDraft } from "./EventDialog";
import { EventPane } from "./EventPane";
import { AgendaView, MonthView, WeekView } from "./Views";

/** What the address holds: the view, the day looked at, the open event. */
export interface ScheduleSearch {
  view?: View;
  date?: Ymd;
  event?: string;
}

export interface CalendarScreenProps {
  workspaceId: string;
  search: ScheduleSearch;
  onSearch: (next: ScheduleSearch) => void;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/** The time, ticking once a minute (the line in the week, "Today"). */
function useClock(fixed: Date | undefined): Date {
  const [now, setNow] = useState(() => fixed ?? new Date());
  useEffect(() => {
    if (fixed !== undefined) {
      return;
    }
    const id = setInterval(() => {
      setNow(new Date());
    }, 60_000);
    return () => {
      clearInterval(id);
    };
  }, [fixed]);
  return fixed ?? now;
}

/**
 * The Calendar place: Agenda, Week or Month of the member's events (what
 * Scout read from their mail, typed and linked to its email, and what their
 * calendars hold), the event's sheet beside it, and the keys for all of it.
 */
export function CalendarScreen({ workspaceId, search, onSearch, now }: CalendarScreenProps) {
  const { t: typed, i18n } = useTranslation("schedule");
  const t: T = typed;
  const locale = i18n.language;
  const settings = useScheduleSettings(workspaceId);
  const calendars = useCalendars(workspaceId);
  const zone = settings.data?.effective_time_zone ?? "UTC";
  const clock = nowIn(zone, useClock(now));
  const view = search.view ?? "agenda";
  const anchor = search.date ?? clock.day;
  const period = windowOf(view, anchor);
  const days = daysOf(period);
  const events = useEvents(workspaceId, period, { enabled: settings.data !== undefined });
  const due = useDueItems(workspaceId);
  const actions = useEventActions(workspaceId);
  const dueActions = useDueActions(workspaceId);
  const [draft, setDraft] = useState<EventDraft | undefined>();

  const go = (next: ScheduleSearch) => {
    onSearch({
      ...(next.view === undefined || next.view === "agenda" ? {} : { view: next.view }),
      ...(next.date === undefined || next.date === clock.day ? {} : { date: next.date }),
      ...(next.event === undefined ? {} : { event: next.event }),
    });
  };
  const changeView = (next: View) => {
    go({ view: next, date: anchor });
  };
  const move = (direction: 1 | -1) => {
    go({ view, date: shift(view, anchor, direction) });
  };
  const newEvent = () => {
    setDraft({ day: anchor });
  };

  useShortcut("schedule.view.agenda", () => {
    changeView("agenda");
  });
  useShortcut("schedule.view.week", () => {
    changeView("week");
  });
  useShortcut("schedule.view.month", () => {
    changeView("month");
  });
  useShortcut("schedule.today", () => {
    go({ view });
  });
  useShortcut(
    "list.next",
    () => {
      move(1);
    },
    { alternates: false, enabled: draft === undefined },
  );
  useShortcut(
    "list.previous",
    () => {
      move(-1);
    },
    { alternates: false, enabled: draft === undefined },
  );
  useShortcut("schedule.create_event", newEvent, { enabled: draft === undefined });

  const list = events.data?.events ?? [];
  const calendarList = calendars.data ?? [];
  const selectedEvent =
    search.event === undefined ? undefined : list.find((each) => eventKey(each) === search.event);
  const selectedDue =
    search.event === undefined ? undefined : due.find((each) => dueKeyOf(each) === search.event);
  const select = (key: string) => {
    go({ view, date: anchor, event: key });
  };
  const close = () => {
    go({ view, date: anchor });
  };

  const last = addDays(period.to, -1);
  const periodLabel = `${
    view === "month"
      ? monthTitle(locale, anchor)
      : t("when.range", { start: daySub(locale, period.from), end: daySub(locale, last) })
  } · ${t("period.zone", { city: cityOf(zone) })}`;
  const unit = view === "month" ? "Month" : "Week";

  const create = (body: NewEvent) => {
    actions.create.mutate(body, {
      onSuccess: (created) => {
        setDraft(undefined);
        go({ view, date: anchor, event: eventKey(created) });
      },
    });
  };

  let body: ReactNode;
  if (events.isError || settings.isError) {
    body = (
      <div className="p-4">
        <Banner
          tone="danger"
          title={t("loadError")}
          action={
            <Button
              onClick={() => {
                void events.refetch();
                void settings.refetch();
              }}
            >
              {t("retry")}
            </Button>
          }
        />
      </div>
    );
  } else if (events.data === undefined || settings.data === undefined) {
    body = <SkeletonList rows={5} />;
  } else {
    const shared = {
      days,
      events: list,
      due,
      calendars: calendarList,
      today: clock,
      selected: search.event,
      onSelect: select,
      onOpenDay: (day: Ymd) => {
        go({ view: "agenda", date: day });
      },
    };
    body =
      view === "week" ? (
        <WeekView {...shared} />
      ) : view === "month" ? (
        <MonthView {...shared} />
      ) : (
        <AgendaView {...shared} />
      );
  }

  return (
    <>
      <ReaderPane label={t("title")}>
        <CalendarHeader
          title={t("title")}
          period={periodLabel}
          views={
            <ViewTabs
              label={t("views.label")}
              value={view}
              options={VIEWS.map((each) => ({
                value: each,
                label: t(`views.${each}`),
                shortcut: `schedule.view.${each}` as const,
              }))}
              onChange={changeView}
            />
          }
          actions={
            <OverflowActions
              label={t("toolbar.label")}
              actions={[]}
              end={[
                {
                  id: "previous",
                  label: t(`toolbar.previous${unit}`),
                  icon: "chevron-left",
                  shortcut: "list.previous",
                  onSelect: () => {
                    move(-1);
                  },
                },
                {
                  id: "today",
                  label: t("toolbar.today"),
                  icon: "schedule",
                  shortcut: "schedule.today",
                  onSelect: () => {
                    go({ view });
                  },
                },
                {
                  id: "next",
                  label: t(`toolbar.next${unit}`),
                  icon: "chevron-right",
                  shortcut: "list.next",
                  onSelect: () => {
                    move(1);
                  },
                },
                {
                  id: "new",
                  label: t("toolbar.newEvent"),
                  icon: "plus",
                  shortcut: "schedule.create_event",
                  onSelect: newEvent,
                },
              ]}
            />
          }
        />
        {body}
      </ReaderPane>
      {selectedEvent === undefined ? null : (
        <EventPane
          key={eventKey(selectedEvent)}
          workspaceId={workspaceId}
          event={selectedEvent}
          calendars={calendarList}
          lastPushed={settings.data?.last_push_calendar}
          zone={zone}
          actions={actions}
          onClose={close}
          onEdit={() => {
            setDraft({ event: selectedEvent, day: anchor });
          }}
        />
      )}
      {selectedDue === undefined ? null : (
        <DuePane
          key={dueKeyOf(selectedDue)}
          workspaceId={workspaceId}
          item={selectedDue}
          today={clock.day}
          act={dueActions}
          onClose={close}
        />
      )}
      {draft === undefined ? null : (
        <EventDialog
          draft={draft}
          zone={zone}
          saving={actions.create.isPending || actions.edit.isPending}
          failed={actions.create.isError || actions.edit.isError}
          onCreate={create}
          onEdit={(event, changes) => {
            actions.edit.mutate(
              { event, changes },
              {
                onSuccess: () => {
                  setDraft(undefined);
                },
              },
            );
          }}
          onClose={() => {
            actions.create.reset();
            actions.edit.reset();
            setDraft(undefined);
          }}
        />
      )}
    </>
  );
}
