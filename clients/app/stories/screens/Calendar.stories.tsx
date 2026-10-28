import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { INBOX, LEDGER, NDA, PRIMARY, Q4, STREAM_COUNTS, W } from "../../test/demo/conversations";
import { CONNECTION, ZOHO_CONNECTION } from "../../test/demo/identity";
import {
  CALENDARS,
  CAMPBOOKS,
  DENTIST,
  DENTIST_READ,
  FLIGHT,
  HANDOVER,
  HANDOVER_ITEM,
  LEASE,
  LEASE_DETAIL,
  LEASE_MESSAGE,
  READ_DONE,
  READ_FALLBACK,
  READ_UNREAD,
  REVIEW,
  SCHEDULE_NOW,
  SHARED,
  TASKS,
  WEEK_EVENTS,
  taskSources,
  conversationId,
  messageId,
} from "../../test/demo/schedule";
import { ANA, DUARTE, SESSION, SOFIA } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";
import { pinClock, scheduleApi } from "./schedule-mock";

// The Calendar place, the event and task sheets, the "Scout noticed" strip in
// the reader and Settings → Calendar, in the real app (router, shell, sidebar)
// against a demo week: Ana Duarte's, from Monday 26 October 2026, "today"
// being Tuesday 27 October at 14:20 in Lisbon. The schedule API is stateful:
// removing, answering and adding change what the next list returns.

const BASE = `/w/${DUARTE.id}`;
const SCHEDULE = `${BASE}/schedule`;

/** The session, workspace and shell calls every screen of the app makes. */
function shell(): Record<string, MockRoute> {
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/members`]: { body: { members: [ANA, SOFIA] } },
  };
}

function AppAt({ path }: { path: string }) {
  const [app] = useState(() => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const router = createAppRouter(queryClient, createMemoryHistory({ initialEntries: [path] }));
    return { queryClient, router };
  });
  return (
    <QueryClientProvider client={app.queryClient}>
      <UndoProvider>
        <RouterProvider router={app.router} />
      </UndoProvider>
    </QueryClientProvider>
  );
}

interface ScreenArgs {
  path: string;
  /** Builds the API anew for each run of the story, so its state starts fresh. */
  api: () => Record<string, MockRoute>;
}

const meta = {
  title: "Screens/Calendar",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    const unpin = pinClock(SCHEDULE_NOW);
    const restore = mockFetch({ ...shell(), ...args.api() });
    return () => {
      restore();
      unpin();
    };
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * Ana's open tasks, as the tasks API keeps them: Done takes a task off the
 * list (and Undo puts it back), so the next read of the list is true; and the
 * emails they came from, for the source in a row and its sheet.
 */
function tasksApi(list: readonly (typeof TASKS)[number][] = TASKS): Record<string, MockRoute> {
  let open = [...list];
  const routes: Record<string, MockRoute> = {
    [`GET ${W}/tasks`]: () => ({ body: { tasks: open } }),
    [`GET ${W}/conversations`]: (_body, url) => {
      const ids = new Set((url.searchParams.get("ids") ?? "").split(","));
      return {
        body: {
          conversations: taskSources().filter((mail) => ids.has(mail.id)),
          next_cursor: null,
        },
      };
    },
  };
  for (const each of list) {
    routes[`POST ${W}/tasks/${each.id}/done`] = () => {
      open = open.filter((other) => other.id !== each.id);
      return { body: { ...each, state: "done", revision: each.revision + 1 } };
    };
    routes[`POST ${W}/tasks/${each.id}/reopen`] = () => {
      open = [...open, each];
      return { body: { ...each, revision: each.revision + 2 } };
    };
  }
  return routes;
}

/** Ana's free time after now, for a task sheet's Hold time. */
const FREE_TIME = [
  { starts: "2026-10-28T09:00:00+00:00", ends: "2026-10-28T12:00:00+00:00" },
  { starts: "2026-10-29T14:00:00+00:00", ends: "2026-10-29T15:00:00+00:00" },
];

/** The week's schedule API, with the demo events (or `events`) and Ana's tasks (or `tasks`). */
const calendarApi =
  (events = WEEK_EVENTS, tasks = TASKS) =>
  () => {
    const mock = scheduleApi(events);
    return {
      ...mock.routes,
      ...tasksApi(tasks),
      [`GET ${W}/schedule/free-slots`]: {
        body: { slots: FREE_TIME, calendars: [], time_zone: "Europe/Lisbon" },
      },
      // What the model read from the dentist's email: the sentence in its sheet.
      [`GET ${W}/conversations/${conversationId(1)}/messages/${messageId(1)}/read`]: {
        body: DENTIST_READ,
      },
    };
  };

/** A viewport the story test runs at (the addon reads `globals.viewport`). */
function at(name: string, width: number, height: number) {
  return {
    parameters: {
      viewport: {
        options: {
          [name]: { name, styles: { width: `${String(width)}px`, height: `${String(height)}px` } },
        },
      },
    },
    globals: { viewport: { value: name } },
  };
}

/** No view in the address: the Calendar opens on Week on a desktop. */
export const DefaultDesktop: Story = {
  args: { path: SCHEDULE, api: calendarApi() },
  play: async ({ canvasElement }) => {
    await expect(await within(canvasElement).findByRole("region", { name: "Week" })).toBeVisible();
  },
};

/** No view in the address: the Calendar opens on Agenda on a phone. */
export const DefaultPhone: Story = {
  args: { path: SCHEDULE, api: calendarApi() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(DENTIST.title);
    await expect(canvas.queryByRole("region", { name: "Week" })).toBeNull();
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};

/** Agenda: the week's days from today, a flight's sheet beside them. */
export const AgendaDesktop: Story = {
  args: { path: `${SCHEDULE}?view=agenda&event=${FLIGHT.id}`, api: calendarApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect((await canvas.findAllByText(FLIGHT.title)).length).toBeGreaterThan(1);
    await expect(await canvas.findByText("Booking confirmed: Lisbon → Amsterdam")).toBeVisible();
    await expect(canvas.getAllByText("Overdue · 2 days").length).toBeGreaterThan(0);
  },
};

/** Week: all-day lane, Due lane, the now line, events side by side. */
export const WeekDesktop: Story = {
  args: { path: `${SCHEDULE}?view=week&event=${DENTIST.id}`, api: calendarApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("region", { name: "Week" })).toBeVisible();
    await expect((await canvas.findAllByText(DENTIST.title)).length).toBeGreaterThan(1);
  },
};

/** Week on a phone: the page does not scroll sideways, the grid scrolls inside itself. */
export const WeekPhone: Story = {
  args: { path: `${SCHEDULE}?view=week`, api: calendarApi() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("region", { name: "Week" });
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};

/** Agenda on a phone: the week strip, one row per event, a mail mark for Scout's. */
export const AgendaPhone: Story = {
  args: { path: `${SCHEDULE}?view=agenda`, api: calendarApi() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(DENTIST.title);
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};

/** An event's sheet fills the phone's screen. */
export const SheetPhone: Story = {
  args: { path: `${SCHEDULE}?view=agenda&event=${FLIGHT.id}`, api: calendarApi() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("complementary", { name: FLIGHT.title });
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};

/**
 * An event Scout read from an email: its kind (changeable), when, the email it
 * came from, the sentence it read the date from, and Add to my calendar.
 */
export const EventSheet: Story = {
  args: { path: `${SCHEDULE}?view=agenda&event=${DENTIST.id}`, api: calendarApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: DENTIST.title });
    const inSheet = within(sheet);
    await expect(await inSheet.findByText("Your appointment is confirmed")).toBeVisible();
    await expect(
      await inSheet.findByText("Your appointment is confirmed for Tuesday 27 October at 15:00."),
    ).toBeVisible();
    await expect(inSheet.getByRole("button", { name: /Add to my calendar/u })).toBeVisible();
    await expect(inSheet.getByRole("button", { name: /Edit/u })).toBeVisible();
  },
};

/** Remove, with Undo in the toast for ten seconds. */
export const RemoveWithUndo: Story = {
  args: { path: `${SCHEDULE}?view=agenda&event=${DENTIST.id}`, api: calendarApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: DENTIST.title });
    await userEvent.click(within(sheet).getByRole("button", { name: /Remove/u }));
    await expect(await canvas.findByText(/Dentist check-up removed/u)).toBeVisible();
    await expect(canvas.getByRole("button", { name: /Undo/u })).toBeVisible();
  },
};

/** An invitation: Accept / Maybe / Decline, and the change Google refused. */
export const InvitationSheet: Story = {
  args: { path: `${SCHEDULE}?view=agenda&event=${REVIEW.id}`, api: calendarApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: REVIEW.title });
    await expect(
      within(sheet).getByText(/Changed in Google meanwhile; we kept theirs/u),
    ).toBeVisible();
    await userEvent.click(within(sheet).getByRole("button", { name: "Accept" }));
    await waitFor(() =>
      expect(within(sheet).getByRole("button", { name: "Accept" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
  },
};

const [EDP, COSTA, LEASE_TASK] = TASKS;

/** A task due today (and planned for today, so one row): Paid (E), Open task (↵), no Hold time, Later or Remove. */
export const TaskSheet: Story = {
  args: {
    path: `${SCHEDULE}?view=agenda&event=due:${COSTA?.id ?? ""}`,
    api: calendarApi(),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: /Pay · Costa Obras invoice/u });
    const inSheet = within(sheet);
    await expect(inSheet.getByRole("button", { name: /Paid/u })).toBeVisible();
    await expect(inSheet.getByRole("button", { name: /Open task/u })).toBeVisible();
    await expect(inSheet.queryByRole("button", { name: /Hold time/u })).toBeNull();
    await expect(inSheet.queryByRole("button", { name: /Later/u })).toBeNull();
    await expect(inSheet.queryByRole("button", { name: /Remove/u })).toBeNull();
    await expect(await inSheet.findByText("Invoice 2026-118, works at Rua da Prata")).toBeVisible();
  },
};

/** A task on the day it was planned for: the Planned chip, the plan and the due day. */
export const TaskSheetPlanned: Story = {
  args: {
    path: `${SCHEDULE}?view=agenda&date=2026-10-28&event=plan:${LEASE_TASK?.id ?? ""}`,
    api: calendarApi(),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: /Do · Send the lease/u });
    const inSheet = within(sheet);
    await expect(inSheet.getByText("Planned")).toBeVisible();
    await expect(inSheet.getByText(/^Planned for .*28/u)).toBeVisible();
    await expect(inSheet.getByText(/^Due .*29/u)).toBeVisible();
  },
};

/** An overdue task: marked in the danger colour; Paid takes it off with an Undo. */
export const TaskSheetOverdue: Story = {
  args: {
    path: `${SCHEDULE}?view=agenda&event=due:${EDP?.id ?? ""}`,
    api: calendarApi(),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: /Pay · EDP bill/u });
    await userEvent.click(within(sheet).getByRole("button", { name: /Paid/u }));
    await expect(await canvas.findByText(/Paid · EDP bill/u)).toBeVisible();
    await waitFor(() =>
      expect(canvas.queryByRole("button", { name: /Pay · EDP bill/u })).toBeNull(),
    );
  },
};

/** A Do task's sheet offers Hold time (H): three free slots, Length and Calendar. */
export const TaskSheetHoldTime: Story = {
  args: {
    path: `${SCHEDULE}?view=agenda&date=2026-10-28&event=plan:${LEASE_TASK?.id ?? ""}`,
    api: calendarApi(),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: /Do · Send the lease/u });
    await userEvent.click(within(sheet).getByRole("button", { name: /Hold time/u }));
    const slots = await within(sheet).findByRole("group", { name: "Free time" });
    await expect(await within(slots).findAllByRole("button")).toHaveLength(3);
  },
};

/** "Send the lease" held Wednesday 10:00–10:30, due Thursday: the held block, no Planned row. */
const HELD_BLOCK = {
  ...(WEEK_EVENTS.find((event) => event.kind === "hold") ?? REVIEW),
  id: "0192f1a0-0000-7000-8000-0000000042e1",
  title: "Send the lease",
  starts: "2026-10-28T10:00:00+00:00",
  ends: "2026-10-28T10:30:00+00:00",
  calendar_id: CAMPBOOKS.id,
  on_provider: false,
  source: { type: "tasks.task", id: LEASE_TASK?.id ?? "" },
};
const HELD_TASKS = TASKS.map((task) =>
  task.id === LEASE_TASK?.id
    ? {
        ...task,
        hold: {
          event_id: HELD_BLOCK.id,
          calendar_id: CAMPBOOKS.id,
          starts: HELD_BLOCK.starts,
          ends: HELD_BLOCK.ends,
        },
      }
    : task,
);

/** The week with a held task: its Held block on Wednesday opens the task, and Thursday its Due row. */
export const WeekWithHeldTask: Story = {
  args: {
    path: `${SCHEDULE}?view=agenda&date=2026-10-28&event=${HELD_BLOCK.id}`,
    api: calendarApi([...WEEK_EVENTS, HELD_BLOCK], HELD_TASKS),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: "Send the lease" });
    await expect(within(sheet).getByRole("button", { name: /Open task/u })).toBeVisible();
  },
};

/** The same week, idle (for the recording). */
export const WeekWithHeldTaskIdle: Story = {
  args: {
    path: `${SCHEDULE}?view=week`,
    api: calendarApi([...WEEK_EVENTS, HELD_BLOCK], HELD_TASKS),
  },
};

/** The week with Ana's tasks: dashed Due pills, solid Planned pills, no sheet (for the recording). */
export const WeekWithTasksIdle: Story = {
  args: { path: `${SCHEDULE}?view=week`, api: calendarApi() },
};

/** The same week on a phone, as an Agenda, idle. */
export const AgendaWithTasksIdle: Story = {
  args: { path: `${SCHEDULE}?view=agenda`, api: calendarApi() },
  ...at("phone", 375, 812),
};

/** The month with its tasks after each day's events, idle. */
export const MonthWithTasksIdle: Story = {
  args: { path: `${SCHEDULE}?view=month`, api: calendarApi() },
};

/** Month: each day's first events, "+N more"; a dot per event on a phone. */
export const MonthDesktop: Story = {
  args: { path: `${SCHEDULE}?view=month`, api: calendarApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("group", { name: "Month" })).toBeVisible();
  },
};

export const MonthPhone: Story = {
  args: { path: `${SCHEDULE}?view=month`, api: calendarApi() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("group", { name: "Month" });
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};

/** Nothing ahead: one line. */
export const Empty: Story = {
  args: {
    path: `${SCHEDULE}?view=agenda`,
    api: () => ({ ...calendarApi([])(), [`GET ${W}/tasks`]: { body: { tasks: [] } } }),
  },
  play: async ({ canvasElement }) => {
    await expect(await within(canvasElement).findByText("Nothing on your calendar.")).toBeVisible();
  },
};

/** New event (N, then E): the form, with the day looked at. */
export const NewEvent: Story = {
  args: { path: `${SCHEDULE}?view=agenda`, api: calendarApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(DENTIST.title);
    await userEvent.click(canvas.getByRole("button", { name: /New event/u }));
    await expect(await within(document.body).findByRole("dialog")).toBeInTheDocument();
  },
};

/* What Scout noticed, in the reader. */

/** The inbox around the lease email, with its read in `read`. */
function reader(
  read: () => Record<string, MockRoute>,
  calendars: readonly (typeof CAMPBOOKS)[] = [CAMPBOOKS],
) {
  return () => {
    const conversationRoutes: Record<string, MockRoute> = {
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
      [`GET ${W}/conversations`]: {
        body: { conversations: [LEASE, Q4, NDA, LEDGER, ...PRIMARY.slice(3)], next_cursor: null },
      },
      [`GET ${W}/conversations/${LEASE.id}`]: { body: LEASE_DETAIL },
      [`POST ${W}/conversations/${LEASE.id}/read`]: { body: { ...LEASE, unread_count: 0 } },
    };
    return {
      // No provider calendar is connected yet: no "Add to my calendar" here.
      ...scheduleApi([...WEEK_EVENTS, HANDOVER], { calendars }).routes,
      ...conversationRoutes,
      ...read(),
    };
  };
}

const READ = `${W}/conversations/${LEASE.id}/messages/${LEASE_MESSAGE}/read`;

/** Scout read the email: the handover is on the calendar, with Add to my calendar and Undo. */
export const ReaderScoutRead: Story = {
  args: {
    path: `${BASE}/inbox/primary/${LEASE.id}`,
    api: reader(() => ({ [`GET ${READ}`]: { body: READ_DONE } })),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const strip = await canvas.findByRole("region", { name: "Scout noticed" });
    await expect(within(strip).getByText(HANDOVER_ITEM.title)).toBeVisible();
    await expect(await within(strip).findByText("On your Campbooks calendar")).toBeVisible();
    await expect(await within(strip).findByRole("button", { name: /Undo/u })).toBeVisible();
  },
};

/** Opened before it was read: "Scout is reading this…", then the handover, without a reload. */
export const ReaderReadingOnOpen: Story = {
  args: {
    path: `${BASE}/inbox/primary/${LEASE.id}`,
    api: reader(() => {
      let state = READ_UNREAD;
      return {
        [`GET ${READ}`]: () => ({ body: state }),
        [`POST ${READ}`]: () => {
          state = { ...READ_UNREAD, status: "queued" };
          setTimeout(() => {
            state = READ_DONE;
          }, 6000);
          return { body: { outcome: "queued" } };
        },
      };
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Scout is reading this…")).toBeVisible();
  },
};

/** No model for reading on this server: "This email has an appointment", with Add. */
export const ReaderNoModel: Story = {
  args: {
    path: `${BASE}/inbox/primary/${LEASE.id}`,
    api: reader(() => ({ [`GET ${READ}`]: { body: READ_FALLBACK } })),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const strip = await canvas.findByRole("region", { name: "Scout noticed" });
    await expect(within(strip).getByText("This email has an appointment")).toBeVisible();
    await userEvent.click(within(strip).getByRole("button", { name: "Add" }));
    await expect(await within(document.body).findByRole("dialog")).toBeInTheDocument();
  },
};

export const ReaderScoutReadPhone: Story = {
  args: {
    path: `${BASE}/inbox/primary/${LEASE.id}`,
    api: reader(() => ({ [`GET ${READ}`]: { body: READ_DONE } })),
  },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByRole("region", { name: "Scout noticed" });
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};

/* Settings → Calendar. */

const problem = (status: number, code: string): MockRoute => ({
  status,
  body: { type: "about:blank", title: "Problem", status, code },
});

const CALENDAR_ACCESS = {
  ...CONNECTION,
  capabilities: ["messages", "calendar"],
  offers: [],
} as const;
/** A Google mailbox signed in for mail only: Settings offers it calendar access. */
const GOOGLE_MAIL_ONLY = { ...CONNECTION, capabilities: ["messages"] } as const;
/** A Zoho mailbox, mail only: the server does not offer it calendar access (its adapter is off). */
const MAIL_ONLY = {
  ...ZOHO_CONNECTION,
  address: "ana@lopesimoveis.example",
  health: "ok",
  capabilities: ["messages"],
  offers: [],
} as const;
/** The same Zoho mailbox on a server whose Zoho Calendar adapter is on. */
const ZOHO_OFFERED = { ...MAIL_ONLY, offers: ["calendar"] } as const;

function settingsApi() {
  return {
    ...scheduleApi(undefined, { calendars: [...CALENDARS, SHARED] }).routes,
    "GET /bff/api/v1/identity/held-grants": { body: { items: [] } },
    "GET /bff/api/v1/identity/passkeys": { body: { items: [] } },
    "GET /bff/api/v1/identity/ways-in": {
      body: { items: [], providers: ["google", "zoho"], mailbox_providers: ["google", "zoho"] },
    },
    [`GET ${W}/connections`]: { body: { connections: [CALENDAR_ACCESS, MAIL_ONLY] } },
  } satisfies Record<string, MockRoute>;
}

/** Calendars on or off, the kinds Scout adds and where else each goes, time zone, working hours. */
export const Settings: Story = {
  args: { path: `${BASE}/settings/calendar`, api: settingsApi },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { name: "Calendars" })).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Add calendar access" })).toBeNull();
    // Mine, Following and Off: each provider calendar says which it is.
    const group = (name: string) => canvas.getByRole("list", { name });
    await expect(within(group("Mine")).getByText("Work")).toBeVisible();
    await expect(within(group("Following")).getByText("Marta Lopes")).toBeVisible();
    await expect(within(group("Following")).getByText(/Shared with you/)).toBeVisible();
    await expect(within(group("Off")).getByText("Holidays in Portugal")).toBeVisible();
    // Following a calendar the member owns moves it at once, and Undo puts it back.
    await userEvent.click(
      within(canvas.getByRole("group", { name: "Role of Work" })).getByRole("button", {
        name: "Following",
      }),
    );
    await waitFor(() => expect(within(group("Following")).getByText("Work")).toBeVisible());
    await userEvent.click(await canvas.findByRole("button", { name: /Undo/u }));
    await waitFor(() => expect(within(group("Mine")).getByText("Work")).toBeVisible());
    await userEvent.click(canvas.getByRole("switch", { name: "Scout adds Stays" }));
    await waitFor(() =>
      expect(canvas.getByRole("switch", { name: "Scout adds Stays" })).toHaveAttribute(
        "aria-checked",
        "false",
      ),
    );
  },
};

function addAccessApi() {
  return {
    ...settingsApi(),
    [`GET ${W}/connections`]: { body: { connections: [GOOGLE_MAIL_ONLY, MAIL_ONLY] } },
  } satisfies Record<string, MockRoute>;
}

/** A Google mailbox without calendar access: Add calendar access, and not for the Zoho one (not offered). */
export const SettingsAddAccess: Story = {
  args: { path: `${BASE}/settings/calendar`, api: addAccessApi },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("button", { name: "Add calendar access" })).toBeVisible();
    await expect(canvas.getAllByRole("button", { name: "Add calendar access" })).toHaveLength(1);
  },
};

function zohoOfferedApi() {
  return {
    ...settingsApi(),
    [`GET ${W}/connections`]: { body: { connections: [GOOGLE_MAIL_ONLY, ZOHO_OFFERED] } },
  } satisfies Record<string, MockRoute>;
}

/** The server offers the Zoho mailbox calendar access too: one button for each. */
export const SettingsAddAccessZoho: Story = {
  args: { path: `${BASE}/settings/calendar`, api: zohoOfferedApi },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findAllByRole("button", { name: "Add calendar access" }),
    ).toHaveLength(2);
    await expect(canvas.getByText("ana@lopesimoveis.example")).toBeVisible();
  },
};

function stepUpApi() {
  return {
    ...addAccessApi(),
    "POST /bff/api/v1/identity/links": problem(403, "step_up_required"),
  } satisfies Record<string, MockRoute>;
}

/** The sign-in is too old for a reconnect: the page offers Sign in again, not a dead end. */
export const SettingsStepUp: Story = {
  args: { path: `${BASE}/settings/calendar`, api: stepUpApi },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Add calendar access" }));
    await expect(await canvas.findByText("Sign in again first")).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Sign in again" })).toBeVisible();
    await expect(canvas.queryByText("Couldn’t start that. Try again.")).toBeNull();
  },
};

function mismatchApi() {
  return {
    ...settingsApi(),
    [`GET ${W}/connections`]: { body: { connections: [CALENDAR_ACCESS, ZOHO_OFFERED] } },
  } satisfies Record<string, MockRoute>;
}

/** Zoho answered as another account: the page names the address to switch to. */
export const SettingsAccountMismatch: Story = {
  args: { path: `${BASE}/settings/calendar?link_error=account_mismatch`, api: mismatchApi },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText(
        "Zoho answered as another account. Switch to ana@lopesimoveis.example at Zoho, then try again.",
      ),
    ).toBeVisible();
  },
};

export const SettingsPhone: Story = {
  args: { path: `${BASE}/settings/calendar`, api: settingsApi },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByRole("heading", { name: "Calendars" });
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};

/* Twins without a `play`, for the recording (scenarios/calendar.ts): a story
   with a play acts on load, and the recording does the acting itself. */

/** The Calendar as a member finds it, to drive by hand: Week on a desktop, Agenda on a phone. */
export const CalendarIdle: Story = {
  args: { path: SCHEDULE, api: calendarApi() },
};

/** The same, at 375px: the address names no view and the Calendar opens on Agenda. */
export const CalendarIdlePhone: Story = {
  args: { path: SCHEDULE, api: calendarApi() },
  ...at("phone", 375, 812),
};

/** The lease email, read, to drive by hand. */
export const ReaderIdle: Story = {
  args: {
    path: `${BASE}/inbox/primary/${LEASE.id}`,
    api: reader(() => ({ [`GET ${READ}`]: { body: READ_DONE } })),
  },
};

/** Settings → Calendar, to drive by hand. */
export const SettingsIdle: Story = {
  args: { path: `${BASE}/settings/calendar`, api: settingsApi },
};

/** Settings → Calendar with Mine, Following (shared with you) and Off, to drive by hand. */
export const SettingsRolesIdle: Story = {
  args: { path: `${BASE}/settings/calendar`, api: settingsApi },
};

/** The same at 375px. */
export const SettingsRolesIdlePhone: Story = {
  args: { path: `${BASE}/settings/calendar`, api: settingsApi },
  ...at("phone", 375, 812),
};

/** Settings with Add calendar access, to drive by hand. */
export const SettingsAddAccessIdle: Story = {
  args: { path: `${BASE}/settings/calendar`, api: addAccessApi },
};

/** Settings with Add calendar access for Google and Zoho, whose sign-in is too old: to drive by hand. */
export const SettingsStepUpIdle: Story = {
  args: {
    path: `${BASE}/settings/calendar`,
    api: () => ({
      ...stepUpApi(),
      [`GET ${W}/connections`]: { body: { connections: [GOOGLE_MAIL_ONLY, ZOHO_OFFERED] } },
    }),
  },
};

/** The same refusal, to drive by hand (the recording). */
export const SettingsAccountMismatchIdle: Story = {
  args: { path: `${BASE}/settings/calendar?link_error=account_mismatch`, api: mismatchApi },
};

/** The invitation Google refused a change to, to drive by hand. */
export const InvitationSheetIdle: Story = {
  args: { path: `${SCHEDULE}?view=agenda&event=${REVIEW.id}`, api: calendarApi() },
};

/** The lease email, read, with Google calendars connected: Add to my calendar. */
export const ReaderGoogleIdle: Story = {
  args: {
    path: `${BASE}/inbox/primary/${LEASE.id}`,
    api: reader(() => ({ [`GET ${READ}`]: { body: READ_DONE } }), CALENDARS),
  },
};
