import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { api, unwrap } from "~/lib/api";
import { useUndoToast } from "~/lib/ui";

import { useCalendars, useEvents, useScheduleSettings } from "./api/queries";
import type { CalendarView, EventView, Response } from "./api/types";
import { useClock } from "./clock";
import { COMING_DAYS } from "./day";
import { eventKey, whenLong, type T } from "./labels";
import { addDays, dayOf, nowIn, type Ymd } from "./model";

/**
 * Invitations awaiting the member's answer on Home (campbooks-cloud#360):
 * the attention module lists them as Decide items about events; this module
 * reads the events and says each in words, with whether the member is free
 * then, and answers them (`schedule.respond_event`) with Undo.
 */

/** An invitation item: its event and the day it starts (the item's `due_on`). */
export interface InvitationRef {
  id: string;
  on?: string | null | undefined;
}

/** An answer Home offers, in the order of its keys (1, 2, 3). */
export const ANSWERS = ["accepted", "declined", "tentative"] as const satisfies readonly Response[];
export type Answer = (typeof ANSWERS)[number];

/** An invitation as Home's row says it. */
export interface InvitationRow {
  /** The event (`eventKey`). */
  id: string;
  /** The event's title. */
  title: string;
  /** Who asked: the organiser's name, else address, else the calendar's name. */
  who: string;
  /** Whether a person rather than a machine asked (the avatar's kind). */
  person: boolean;
  /** "Invitation: Offsite planning". */
  what: string;
  /** "Thursday 15 October, 14:00 – 15:00 · Office, 3rd floor". */
  excerpt: string;
  /** "You’re free then", or "You have <title> then". */
  tag: string;
  /** Its calendar can be written: the member may answer it from Home. */
  writable: boolean;
  /**
   * The answers, in words, on keys 1 to 3, each with what the row and the
   * toast say once given ("Accepted: Offsite planning"); none on a calendar
   * that cannot be written.
   */
  answers: { id: Answer; label: string; done: string }[];
  /** The Calendar on the event's day, the event open. */
  openInCalendar: () => void;
  /** The event as listed, for the answer. */
  event: EventView;
}

/** Times of an event as instants, for overlaps. */
function span(event: EventView): { start: number; end: number } {
  return { start: new Date(event.starts).getTime(), end: new Date(event.ends).getTime() };
}

/**
 * What the member already has when `invite` happens: the earliest other
 * event that overlaps it and makes them busy (timed, not marked free, not
 * cancelled, not declined; time held for a task counts, as free time counts
 * it). None when they are free then, or the invitation is all-day.
 */
export function busyThen(invite: EventView, events: readonly EventView[]): EventView | undefined {
  if (invite.all_day) {
    return undefined;
  }
  const own = span(invite);
  return events
    .filter(
      (event) =>
        event.id !== invite.id &&
        !event.all_day &&
        event.busy &&
        event.status !== "cancelled" &&
        event.response !== "declined",
    )
    .filter((event) => {
      const other = span(event);
      return other.start < own.end && other.end > own.start;
    })
    .toSorted((a, b) => span(a).start - span(b).start)[0];
}

/**
 * The window Home reads for its calendar and its invitations: today to the
 * day after the last invitation, never less than the week `useHomeDay`
 * reads (so both share one query), never more than the year the server
 * allows.
 */
export function invitationWindow(
  today: Ymd,
  refs: readonly InvitationRef[],
): { from: Ymd; to: Ymd } {
  const week = addDays(today, COMING_DAYS + 1);
  const year = addDays(today, 365);
  const last =
    refs
      .map((ref) => ref.on ?? today)
      .toSorted()
      .at(-1) ?? today;
  const to = addDays(last, 1);
  if (to <= week) {
    return { from: today, to: week };
  }
  return { from: today, to: to > year ? year : to };
}

/** The organiser in words: their name, else address, else the calendar's name. */
function askedBy(event: EventView, calendars: readonly CalendarView[]): string {
  const name = event.organizer?.name?.trim();
  if (name !== undefined && name !== "") {
    return name;
  }
  const address = event.organizer?.address?.trim();
  if (address !== undefined && address !== "") {
    return address;
  }
  return calendars.find((calendar) => calendar.id === event.calendar_id)?.name ?? "";
}

/** "Accepted: Offsite planning", the row's confirmation and the toast. */
function answeredLabel(t: T, answer: Answer, title: string): string {
  return t(`invite.answered.${answer}`, { title });
}

interface InvitationOptions {
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * The invitations of `refs` in words, by event id, read from the member's
 * calendar in their zone and language. An invitation whose event is not
 * listed (gone meanwhile) has no row.
 */
export function useInvitations(
  workspaceId: string,
  refs: readonly InvitationRef[],
  { now }: InvitationOptions = {},
): { rows: ReadonlyMap<string, InvitationRow>; loading: boolean } {
  const { t: typed, i18n } = useTranslation("schedule");
  const t: T = typed;
  const locale = i18n.language;
  const navigate = useNavigate();
  const settings = useScheduleSettings(workspaceId);
  const calendars = useCalendars(workspaceId);
  const zone = settings.data?.effective_time_zone ?? "UTC";
  const clock = nowIn(zone, useClock(now));
  const period = invitationWindow(clock.day, refs);
  const any = refs.length > 0;
  const events = useEvents(workspaceId, period, { enabled: any && settings.data !== undefined });
  const list = events.data?.events ?? [];
  const calendarList = calendars.data ?? [];

  const rows = new Map<string, InvitationRow>();
  for (const ref of refs) {
    // A series: its first occurrence in the window.
    const event = list.find((each) => each.id === ref.id);
    if (event === undefined) {
      continue;
    }
    const calendar = calendarList.find((each) => each.id === event.calendar_id);
    const writable = calendar?.writable === true;
    const busy = busyThen(event, list);
    const place = event.place?.trim() ?? "";
    const when = whenLong(t, locale, event);
    const day = dayOf(event.starts);
    rows.set(ref.id, {
      id: eventKey(event),
      title: event.title,
      who: askedBy(event, calendarList),
      person: event.organizer?.person !== false,
      what: t("invite.title", { title: event.title }),
      excerpt: place === "" ? when : t("invite.where", { when, place }),
      tag: busy === undefined ? t("invite.free") : t("invite.busy", { title: busy.title }),
      writable,
      answers: writable
        ? ANSWERS.map((id) => ({
            id,
            label: t(`rsvp.${id}`),
            done: answeredLabel(t, id, event.title),
          }))
        : [],
      openInCalendar: () => {
        void navigate({
          to: "/w/$workspaceId/schedule",
          params: { workspaceId },
          search: { ...(day === clock.day ? {} : { date: day }), event: eventKey(event) },
        });
      },
      event,
    });
  }
  const loading =
    any &&
    (settings.isPending ||
      calendars.isPending ||
      (settings.data !== undefined && events.isPending && !events.isError));
  return { rows, loading };
}

/** How long an answer's Undo stays: the server's undo window (ten seconds). */
const ANSWER_UNDO_MS = 10_000;

interface AnswerCallbacks {
  /** The answer was taken back: the row comes back. */
  onUndo?: ((eventId: string) => void) | undefined;
  /** The server has the answer, or the answer taken back: what lists it may read again. */
  onChanged?: (() => void) | undefined;
  /** The answer failed: the row comes back. */
  onFailed?: ((eventId: string) => void) | undefined;
}

/**
 * Answering an invitation from Home (`schedule.respond_event`, the
 * Calendar's own operation), with the toast's Undo for the server's undo
 * window: Undo sends the answer before back, so the provider never hears
 * of it.
 */
export function useInvitationAnswers(workspaceId: string, callbacks: AnswerCallbacks = {}) {
  const { t: typed } = useTranslation("schedule");
  const t: T = typed;
  const queryClient = useQueryClient();
  const undo = useUndoToast();
  const respond = async (eventId: string, response: Response) =>
    unwrap(
      await api.POST("/api/v1/w/{workspace_id}/schedule/events/{event_id}/response", {
        params: { path: { workspace_id: workspaceId, event_id: eventId } },
        body: { response },
      }),
    );
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["schedule.list_events"] });
  const mutation = useMutation({
    mutationFn: async ({ event, answer }: { event: EventView; answer: Answer }) => ({
      event,
      answer,
      answered: await respond(event.id, answer),
    }),
    onSuccess: ({ event, answer, answered }) => {
      callbacks.onChanged?.();
      undo.push({
        label: answeredLabel(t, answer, event.title),
        durationMs: ANSWER_UNDO_MS,
        undo: () => {
          callbacks.onUndo?.(event.id);
          void respond(event.id, answered.previous_response)
            .catch(() => {
              undo.fail({ label: t("invite.undoFailed", { title: event.title }) });
            })
            .finally(() => {
              void refresh();
              callbacks.onChanged?.();
            });
        },
      });
    },
    onError: (_error, { event }) => {
      callbacks.onFailed?.(event.id);
      undo.fail({ label: t("invite.failed", { title: event.title }) });
    },
    onSettled: () => refresh(),
  });
  return {
    answer: (event: EventView, answer: Answer) => {
      mutation.mutate({ event, answer });
    },
  };
}
