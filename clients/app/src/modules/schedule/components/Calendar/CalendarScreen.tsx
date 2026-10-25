import { useEffect, useState, useSyncExternalStore, type ReactNode } from "react";
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
import {
  CONVERSATION,
  useTaskActions,
  type TaskSource,
  type TaskView,
  type UseTaskSources,
} from "~/modules/tasks";

import {
  useCalendars,
  useEventActions,
  useEvents,
  useFreeSlots,
  useScheduleSettings,
  useWritableCalendars,
} from "../../api/queries";
import type { NewEvent } from "../../api/types";
import { useTaskRows } from "../../due";
import { eventKey, type T } from "../../labels";
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

/** Where the task sheet's Hold time reads free time and calendars: this module's own. */
const HOLD_SOURCES = { useFreeSlots, useWritableCalendars };

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
  /** Where tasks made from conversations came from (the conversations module's). */
  useSources: UseTaskSources;
}

/** Below 48rem the app is a phone (the tab bar, the single-pane reader). */
const PHONE = "(width < 48rem)";

function subscribePhone(change: () => void): () => void {
  if (typeof globalThis.matchMedia !== "function") {
    return () => {
      // Nothing was listened to.
    };
  }
  const query = globalThis.matchMedia(PHONE);
  query.addEventListener("change", change);
  return () => {
    query.removeEventListener("change", change);
  };
}

const isPhone = () =>
  typeof globalThis.matchMedia === "function" && globalThis.matchMedia(PHONE).matches;

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
export function CalendarScreen({
  workspaceId,
  search,
  onSearch,
  now,
  useSources,
}: CalendarScreenProps) {
  const { t: typed, i18n } = useTranslation("schedule");
  const t: T = typed;
  const locale = i18n.language;
  const settings = useScheduleSettings(workspaceId);
  const calendars = useCalendars(workspaceId);
  const zone = settings.data?.effective_time_zone ?? "UTC";
  const clock = nowIn(zone, useClock(now));
  // No view in the address: Week on a wide screen, Agenda on a phone. A view in it wins.
  const defaultView: View = useSyncExternalStore(subscribePhone, isPhone) ? "agenda" : "week";
  const view = search.view ?? defaultView;
  const anchor = search.date ?? clock.day;
  const period = windowOf(view, anchor);
  const days = daysOf(period);
  const events = useEvents(workspaceId, period, { enabled: settings.data !== undefined });
  const tasks = useTaskRows(workspaceId, clock.day, zone);
  const taskActions = useTaskActions(workspaceId);
  const actions = useEventActions(workspaceId);
  // The emails behind the tasks that came from one.
  const sources = useSources(workspaceId, [
    ...new Set(
      tasks.flatMap(({ task }) => (task.source?.type === CONVERSATION ? [task.source.id] : [])),
    ),
  ]);
  const sourceOf = (task: TaskView): TaskSource | undefined =>
    task.source?.type === CONVERSATION ? sources.get(task.source.id) : undefined;
  const [draft, setDraft] = useState<EventDraft | undefined>();

  const go = (next: ScheduleSearch) => {
    onSearch({
      ...(next.view === undefined || next.view === defaultView ? {} : { view: next.view }),
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
  const selectedTask =
    search.event === undefined ? undefined : tasks.find((each) => each.key === search.event);
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
      tasks,
      sourceOf,
      calendars: calendarList,
      today: clock,
      selected: search.event,
      onSelect: select,
      onOpenDay: (day: Ymd) => {
        // Explicit, so a phone's Agenda and a desktop's both stay in the address.
        onSearch({ view: "agenda", ...(day === clock.day ? {} : { date: day }) });
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
      {selectedTask === undefined ? null : (
        <DuePane
          key={selectedTask.key}
          workspaceId={workspaceId}
          row={selectedTask}
          today={clock.day}
          source={sourceOf(selectedTask.task)}
          done={taskActions.done}
          hold={{ sources: HOLD_SOURCES, run: taskActions.hold }}
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
