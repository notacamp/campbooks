import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, operationKey, unwrap, type Schema } from "~/lib/api";
import { useUndoToast } from "~/lib/ui";
import type { FreeSlots, FreeWindow, HoldCalendar } from "~/modules/tasks";

import { sameMoment, type Period } from "../model";

import type {
  CalendarView,
  EventPatch,
  EventView,
  MessageRead,
  NewEvent,
  Response,
  SettingsView,
} from "./types";

// Keyed by operationId, so the change feed refetches them when an event, a
// calendar or a message changes (`records.gen.ts`).
const keys = {
  events: (workspaceId: string, period: Period) =>
    operationKey("schedule.list_events", {
      workspace_id: workspaceId,
      from: period.from,
      to: period.to,
    }),
  allEvents: () => ["schedule.list_events"] as const,
  calendars: (workspaceId: string) =>
    operationKey("schedule.list_calendars", { workspace_id: workspaceId }),
  settings: (workspaceId: string) =>
    operationKey("schedule.get_settings", { workspace_id: workspaceId }),
  free: (workspaceId: string, window: FreeWindow) =>
    operationKey("schedule.free_slots", {
      workspace_id: workspaceId,
      from: window.from,
      to: window.to,
      minutes: String(window.minutes),
    }),
  read: (workspaceId: string, conversationId: string, messageId: string) =>
    operationKey("conversations.get_message_read", {
      workspace_id: workspaceId,
      conversation_id: conversationId,
      message_id: messageId,
    }),
  conversation: (workspaceId: string, conversationId: string) =>
    operationKey("conversations.get", {
      workspace_id: workspaceId,
      conversation_id: conversationId,
    }),
};

/** The most events the calendar asks for at once (the server's own cap). */
const EVENT_LIMIT = 500;

/** `schedule.list_events`: the events and occurrences in a window, earliest first. */
export function useEvents(workspaceId: string, period: Period, { enabled = true } = {}) {
  return useQuery({
    queryKey: keys.events(workspaceId, period),
    enabled,
    // The week or month you are leaving stays on screen while the next loads.
    placeholderData: keepPreviousData,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/schedule/events", {
          params: {
            path: { workspace_id: workspaceId },
            query: { from: period.from, to: period.to, limit: EVENT_LIMIT },
          },
          signal,
        }),
      ),
  });
}

/** `schedule.list_calendars`: the member's own calendar first, then the others by name. */
export function useCalendars(workspaceId: string) {
  return useQuery({
    queryKey: keys.calendars(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/schedule/calendars", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).calendars,
  });
}

/**
 * `schedule.free_slots`: the member's free time in a window, as Hold time
 * reads it (the tasks module's plug).
 */
export function useFreeSlots(workspaceId: string, window: FreeWindow): FreeSlots {
  const free = useQuery({
    queryKey: keys.free(workspaceId, window),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/schedule/free-slots", {
          params: {
            path: { workspace_id: workspaceId },
            query: { from: window.from, to: window.to, minutes: window.minutes },
          },
          signal,
        }),
      ),
  });
  return { slots: free.data?.slots, failed: free.isError };
}

/**
 * The calendars a hold can go on: the member's writable ones, the primary
 * provider calendar first, then their Campbooks calendar, then the others
 * (the tasks module's plug).
 */
export function useWritableCalendars(workspaceId: string): readonly HoldCalendar[] | undefined {
  const calendars = useCalendars(workspaceId).data;
  if (calendars === undefined) {
    return undefined;
  }
  const rank = (calendar: CalendarView) => {
    if (calendar.source === "provider" && calendar.primary) {
      return 0;
    }
    return calendar.source === "campbooks" ? 1 : 2;
  };
  return calendars
    .filter(
      (calendar) => calendar.writable && (calendar.source === "campbooks" || calendar.syncing),
    )
    .toSorted((a, b) => rank(a) - rank(b))
    .map((calendar) => ({
      id: calendar.id,
      name: calendar.name,
      campbooks: calendar.source === "campbooks",
    }));
}

/** `schedule.get_settings`. */
export function useScheduleSettings(workspaceId: string) {
  return useQuery({
    queryKey: keys.settings(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/schedule/settings", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ),
  });
}

/** `schedule.update_settings` and `schedule.update_calendar`: each shown at once, then confirmed. */
export function useSettingsMutations(workspaceId: string) {
  const queryClient = useQueryClient();
  const settings = useMutation({
    mutationFn: async (patch: Schema<"SettingsPatch">) =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/schedule/settings", {
          params: { path: { workspace_id: workspaceId } },
          body: patch,
        }),
      ),
    onSuccess: (view) => {
      queryClient.setQueryData<SettingsView>(keys.settings(workspaceId), view);
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: keys.settings(workspaceId) }),
  });
  const calendar = useMutation({
    mutationFn: async ({
      calendarId,
      patch,
    }: {
      calendarId: string;
      patch: Schema<"CalendarPatch">;
    }) =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/schedule/calendars/{calendar_id}", {
          params: { path: { workspace_id: workspaceId, calendar_id: calendarId } },
          body: patch,
        }),
      ),
    onSuccess: (view) => {
      queryClient.setQueryData<CalendarView[]>(keys.calendars(workspaceId), (list) =>
        list?.map((each) => (each.id === view.id ? view : each)),
      );
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: keys.calendars(workspaceId) });
      // Turning a calendar on or off changes what the calendar shows.
      await queryClient.invalidateQueries({ queryKey: keys.allEvents() });
    },
  });
  return { settings, calendar };
}

function refreshEvents(queryClient: QueryClient) {
  return queryClient.invalidateQueries({ queryKey: keys.allEvents() });
}

/** Sets one event in every cached window that holds it. */
function replaceEvent(queryClient: QueryClient, event: EventView) {
  queryClient.setQueriesData<Schema<"EventList">>({ queryKey: keys.allEvents() }, (list) =>
    list === undefined
      ? list
      : { ...list, events: list.events.map((each) => (each.id === event.id ? event : each)) },
  );
}

/** How long the Undo of a removal stays (the spec: ten seconds). */
const REMOVE_UNDO_MS = 10_000;

/**
 * What a member does with an event, each with its Undo in the toast (Z
 * takes the latest back): remove, change its kind, add it to a provider
 * calendar, answer an invitation, make one.
 */
export function useEventActions(workspaceId: string) {
  const { t } = useTranslation("schedule");
  const queryClient = useQueryClient();
  const undo = useUndoToast();
  const path = { workspace_id: workspaceId };

  const restore = async (undoToken: string, eventId: string) => {
    unwrap(
      await api.POST("/api/v1/w/{workspace_id}/schedule/events/{event_id}/restore", {
        params: { path: { ...path, event_id: eventId } },
        body: { undo_token: undoToken },
      }),
    );
    await refreshEvents(queryClient);
  };

  const patch = async (eventId: string, body: EventPatch) =>
    unwrap(
      await api.PATCH("/api/v1/w/{workspace_id}/schedule/events/{event_id}", {
        params: { path: { ...path, event_id: eventId } },
        body,
      }),
    );

  const push = async (eventId: string, calendarId: string) =>
    unwrap(
      await api.POST("/api/v1/w/{workspace_id}/schedule/events/{event_id}/push", {
        params: { path: { ...path, event_id: eventId } },
        body: { calendar_id: calendarId },
      }),
    );

  const unpush = async (eventId: string) =>
    unwrap(
      await api.DELETE("/api/v1/w/{workspace_id}/schedule/events/{event_id}/push", {
        params: { path: { ...path, event_id: eventId } },
      }),
    );

  const remove = useMutation({
    mutationFn: async (event: EventView) => {
      const deleted = unwrap(
        await api.DELETE("/api/v1/w/{workspace_id}/schedule/events/{event_id}", {
          params: { path: { ...path, event_id: event.id } },
        }),
      );
      return { event, token: deleted.undo_token };
    },
    onSuccess: ({ event, token }) => {
      undo.push({
        label: t("toast.removed", { title: event.title }),
        durationMs: REMOVE_UNDO_MS,
        undo: () => {
          void restore(token, event.id);
        },
      });
    },
    onSettled: () => refreshEvents(queryClient),
  });

  const edit = useMutation({
    mutationFn: async ({ event, changes }: { event: EventView; changes: EventPatch }) => ({
      event,
      changes,
      updated: await patch(event.id, changes),
    }),
    onSuccess: ({ event, changes, updated }) => {
      replaceEvent(queryClient, updated.event);
      // What the fields were before, for Undo.
      const before: EventPatch = {
        ...(changes.title === undefined ? {} : { title: event.title }),
        ...(changes.kind === undefined ? {} : { kind: event.kind }),
        ...(changes.starts === undefined ? {} : { starts: event.starts }),
        ...(changes.ends === undefined ? {} : { ends: event.ends }),
        ...(changes.place === undefined ? {} : { place: event.place ?? "" }),
      };
      undo.push({
        label:
          changes.kind !== undefined && Object.keys(changes).length === 1
            ? t("toast.kindChanged", {
                title: event.title,
                kind: t(`kind.${updated.event.kind}`),
              })
            : t("toast.saved", { title: updated.event.title }),
        undo: () => {
          void patch(event.id, before).then(() => refreshEvents(queryClient));
        },
      });
    },
    onSettled: () => refreshEvents(queryClient),
  });

  const addTo = useMutation({
    mutationFn: async ({ event, calendar }: { event: EventView; calendar: CalendarView }) => ({
      event,
      calendar,
      pushed: await push(event.id, calendar.id),
    }),
    onSuccess: ({ event, calendar, pushed }) => {
      replaceEvent(queryClient, pushed);
      undo.push({
        label: t("toast.pushed", { calendar: calendar.name }),
        undo: () => {
          void unpush(event.id).then(() => refreshEvents(queryClient));
        },
      });
    },
    onSettled: () => refreshEvents(queryClient),
  });

  const respond = useMutation({
    mutationFn: async ({ event, response }: { event: EventView; response: Response }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/schedule/events/{event_id}/response", {
          params: { path: { ...path, event_id: event.id } },
          body: { response },
        }),
      ),
    onSuccess: (answered, { event }) => {
      replaceEvent(queryClient, answered.event);
      undo.push({
        label: t("toast.answered", {
          title: event.title,
          response: t(`rsvp.${answered.event.response ?? "needs_action"}`),
        }),
        undo: () => {
          void api
            .POST("/api/v1/w/{workspace_id}/schedule/events/{event_id}/response", {
              params: { path: { ...path, event_id: event.id } },
              body: { response: answered.previous_response },
            })
            .then(() => refreshEvents(queryClient));
        },
      });
    },
    onSettled: () => refreshEvents(queryClient),
  });

  const create = useMutation({
    mutationFn: async (body: NewEvent) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/schedule/events", {
          params: { path },
          body,
        }),
      ),
    onSuccess: (event) => {
      undo.push({
        label: t("toast.created", { title: event.title }),
        durationMs: REMOVE_UNDO_MS,
        undo: () => {
          void api
            .DELETE("/api/v1/w/{workspace_id}/schedule/events/{event_id}", {
              params: { path: { ...path, event_id: event.id } },
            })
            .then(() => refreshEvents(queryClient));
        },
      });
    },
    onSettled: () => refreshEvents(queryClient),
  });

  return { remove, edit, addTo, respond, create, restore };
}

/** `conversations.get_message_read`: what Scout read in a message, and whether it is reading. */
export function useMessageRead(
  workspaceId: string,
  conversationId: string,
  messageId: string,
  { enabled = true } = {},
) {
  return useQuery({
    queryKey: keys.read(workspaceId, conversationId, messageId),
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET(
          "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/read",
          {
            params: {
              path: {
                workspace_id: workspaceId,
                conversation_id: conversationId,
                message_id: messageId,
              },
            },
            signal,
          },
        ),
      ),
    // Without a live change feed, a read in progress is asked about again.
    refetchInterval: (query) => (query.state.data?.status === "queued" ? 4000 : false),
  });
}

/** `conversations.request_message_read`: ask for the read of a flagged message, once. */
export function useRequestRead(workspaceId: string, conversationId: string, messageId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () =>
      unwrap(
        await api.POST(
          "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/read",
          {
            params: {
              path: {
                workspace_id: workspaceId,
                conversation_id: conversationId,
                message_id: messageId,
              },
            },
          },
        ),
      ),
    onSettled: () =>
      queryClient.invalidateQueries({
        queryKey: keys.read(workspaceId, conversationId, messageId),
      }),
  });
}

/** What the sheet says about the message an event was read from. */
interface SourceMessage {
  conversationId: string;
  stream: Schema<"Stream">;
  subject: string;
  sender: string;
  sentAt: string | null;
}

/** `conversations.get` for the conversation of an event read from mail. */
export function useSourceMessage(workspaceId: string, event: EventView | undefined) {
  const conversationId = event?.source_conversation ?? undefined;
  const messageId = event?.source?.id;
  return useQuery({
    queryKey: keys.conversation(workspaceId, conversationId ?? ""),
    enabled: conversationId !== undefined,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/conversations/{conversation_id}", {
          params: { path: { workspace_id: workspaceId, conversation_id: conversationId ?? "" } },
          signal,
        }),
      ),
    select: (detail): SourceMessage => {
      const message = detail.messages.find((each) => each.id === messageId);
      const from = message?.from ?? detail.conversation.latest_from;
      return {
        conversationId: detail.conversation.id,
        stream: detail.conversation.stream,
        subject: message?.subject ?? detail.conversation.subject,
        sender: from?.name ?? from?.address ?? "",
        sentAt: message?.sent_at ?? null,
      };
    },
  });
}

/** The sentence Scout read an event's date from: the quote of the matching item of its message's read. */
export function quoteFor(read: MessageRead | undefined, event: EventView): string | undefined {
  const item = read?.items.find(
    (each) => each.title === event.title && sameMoment(each.starts, event.starts),
  );
  return item?.quote ?? undefined;
}
