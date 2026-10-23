import type { Schema } from "~/lib/api";

import { addDays, dayOf, lastDay } from "../../src/modules/schedule/model";
import {
  CALENDARS,
  SETTINGS,
  SOURCE_NUMBERS,
  conversationId,
  sourceDetail,
  WEEK_EVENTS,
} from "../../test/demo/schedule";
import { W } from "../../test/demo/conversations";

import type { MockRoute } from "./mock-fetch";

type Event = Schema<"EventView">;
type Calendar = Schema<"CalendarView">;
type Settings = Schema<"SettingsView">;

/** `fetch` answers of the schedule API, with the state they change (stories only). */
export interface ScheduleMock {
  routes: Record<string, MockRoute>;
  /** The events as they stand now. */
  events: () => Event[];
}

/**
 * The schedule API over in-memory demo data: listing a window, removing an
 * event and bringing it back, changing it, adding it to a calendar and
 * taking it off, answering an invitation, making one, the calendars and the
 * settings. Each call changes the state the next list reads, like a server.
 */
export function scheduleApi(
  initial: readonly Event[] = WEEK_EVENTS,
  { calendars: startCalendars = CALENDARS }: { calendars?: readonly Calendar[] } = {},
): ScheduleMock {
  let events = [...initial];
  let calendars = [...startCalendars];
  let settings: Settings = SETTINGS;
  const removed = new Map<string, Event>();
  let created = 0;
  const S = `${W}/schedule`;

  const find = (eventId: string) => events.find((event) => event.id === eventId);
  const replace = (event: Event) => {
    events = events.map((each) => (each.id === event.id ? event : each));
    return event;
  };

  const routes: Record<string, MockRoute> = {
    [`GET ${S}/calendars`]: () => ({ body: { calendars } }),
    [`GET ${S}/settings`]: () => ({ body: settings }),
    [`PATCH ${S}/settings`]: (body) => {
      settings = { ...settings, ...(body as Partial<Settings>) };
      return { body: settings };
    },
    [`GET ${S}/events`]: (_, url) => {
      const from = url.searchParams.get("from") ?? "";
      const to = url.searchParams.get("to") ?? "";
      const inWindow = events.filter((event) => dayOf(event.starts) < to && lastDay(event) >= from);
      return { body: { events: inWindow, truncated: false } };
    },
    [`POST ${S}/events`]: (body) => {
      const input = body as Partial<Event> & Pick<Event, "title" | "starts">;
      created += 1;
      const event: Event = {
        busy: true,
        on_provider: false,
        item_keys: [],
        origin: "member",
        pending: false,
        recurring: false,
        status: "confirmed",
        all_day: input.starts.length === 10,
        calendar_id: calendars[0]?.id ?? "",
        kind: "none",
        ends: input.ends ?? addDays(dayOf(input.starts), 1),
        id: `0192f1a0-0000-7000-8000-00000000ff${String(created).padStart(2, "0")}`,
        ...input,
      };
      events = [...events, event];
      return { status: 201, body: event };
    },
  };

  for (const calendar of calendars) {
    routes[`PATCH ${S}/calendars/${calendar.id}`] = (body) => {
      const updated = { ...calendar, ...(body as Partial<Calendar>) };
      calendars = calendars.map((each) => (each.id === calendar.id ? updated : each));
      return { body: updated };
    };
  }

  for (const event of initial) {
    const path = `${S}/events/${event.id}`;
    routes[`GET ${path}`] = () => {
      const found = find(event.id);
      return found === undefined
        ? { status: 404, body: { status: 404, code: "not_found", title: "Not found" } }
        : { body: found };
    };
    routes[`DELETE ${path}`] = () => {
      const found = find(event.id);
      if (found !== undefined) {
        removed.set(event.id, found);
        events = events.filter((each) => each.id !== event.id);
      }
      return { body: { id: event.id, undo_token: `undo-${event.id}` } };
    };
    routes[`POST ${path}/restore`] = () => {
      const back = removed.get(event.id);
      if (back === undefined) {
        return { status: 409, body: { status: 409, code: "conflict", title: "Conflict" } };
      }
      events = [...events, back];
      removed.delete(event.id);
      return { body: back };
    };
    routes[`PATCH ${path}`] = (body) => {
      const before = find(event.id) ?? event;
      const changes = body as Partial<Event>;
      const after = replace({ ...before, ...changes });
      return { body: { event: after, id: event.id, kind: before.kind } };
    };
    routes[`POST ${path}/push`] = (body) => {
      const before = find(event.id) ?? event;
      return { body: replace({ ...before, on_provider: true, ...(body as object) }) };
    };
    routes[`DELETE ${path}/push`] = () => {
      const before = find(event.id) ?? event;
      const after = replace({ ...before, on_provider: false });
      return {
        body: { event: after, id: event.id, calendar_id: settings.last_push_calendar ?? "" },
      };
    };
    routes[`POST ${path}/response`] = (body) => {
      const before = find(event.id) ?? event;
      const { response } = body as { response: Event["response"] };
      const after = replace({ ...before, response: response ?? null });
      return { body: { event: after, previous_response: before.response ?? "needs_action" } };
    };
  }

  // The emails behind the events, for "From <subject> · <sender>".
  for (const n of SOURCE_NUMBERS) {
    routes[`GET ${W}/conversations/${conversationId(n)}`] = { body: sourceDetail(n) };
  }

  return { routes, events: () => events };
}

/** Pins `new Date()` and `Date.now()` to `iso` until the returned function is called (stories only). */
export function pinClock(iso: string): () => void {
  const real = globalThis.Date;
  const fixed = new real(iso).getTime();
  globalThis.Date = new Proxy(real, {
    construct(target, args: unknown[], newTarget) {
      return Reflect.construct(target, args.length === 0 ? [fixed] : args, newTarget) as Date;
    },
    get(target, property, receiver) {
      return property === "now"
        ? () => fixed
        : (Reflect.get(target, property, receiver) as unknown);
    },
  });
  return () => {
    globalThis.Date = real;
  };
}
