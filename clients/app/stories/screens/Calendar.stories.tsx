import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { DUE_ACTION_KEY, dueKey, type DueItem } from "../../src/modules/schedule/due";
import { INBOX, LEDGER, NDA, PRIMARY, Q4, STREAM_COUNTS, W } from "../../test/demo/conversations";
import { CONNECTION, ZOHO_CONNECTION } from "../../test/demo/identity";
import {
  CAMPBOOKS,
  DENTIST,
  DENTIST_READ,
  DUE_ITEMS,
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
  WEEK_EVENTS,
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

const asDue = (items: readonly unknown[]) => items as DueItem[];

/** The session, workspace and shell calls every screen of the app makes. */
function shell(): Record<string, MockRoute> {
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/members`]: { body: { members: [ANA, SOFIA] } },
  };
}

function AppAt({ path, due }: { path: string; due: readonly DueItem[] }) {
  const [app] = useState(() => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    // The Due layer's data: the `tasks` contracts publish it; here it is seeded.
    queryClient.setQueryData(dueKey(DUARTE.id), due);
    queryClient.setMutationDefaults(DUE_ACTION_KEY, {
      mutationFn: (variables: { item: DueItem; action: "done" | "hold" | "later" }) => {
        const { item, action } = variables;
        const before = queryClient.getQueryData<DueItem[]>(dueKey(DUARTE.id)) ?? [];
        queryClient.setQueryData(
          dueKey(DUARTE.id),
          before.filter((each) => each.id !== item.id),
        );
        const label = {
          done: item.verb === "pay" ? `${item.title} marked paid` : `${item.title} marked done`,
          hold: `Time held for ${item.title}`,
          later: `${item.title} put off`,
        }[action];
        return Promise.resolve({
          label,
          undo: () => {
            queryClient.setQueryData(dueKey(DUARTE.id), before);
          },
        });
      },
    });
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
  due?: readonly DueItem[];
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
  render: ({ path, due }) => <AppAt path={path} due={due ?? []} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The week's schedule API, with the demo events (or `events`). */
const calendarApi =
  (events = WEEK_EVENTS) =>
  () => {
    const mock = scheduleApi(events);
    return {
      ...mock.routes,
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

const DUE = asDue(DUE_ITEMS);

/** Agenda: the week's days from today, a flight's sheet beside them. */
export const AgendaDesktop: Story = {
  args: { path: `${SCHEDULE}?event=${FLIGHT.id}`, api: calendarApi(), due: DUE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect((await canvas.findAllByText(FLIGHT.title)).length).toBeGreaterThan(1);
    await expect(await canvas.findByText("Booking confirmed: Lisbon → Amsterdam")).toBeVisible();
    await expect(canvas.getAllByText("Overdue · 2 days").length).toBeGreaterThan(0);
  },
};

/** Week: all-day lane, Due lane, the now line, events side by side. */
export const WeekDesktop: Story = {
  args: { path: `${SCHEDULE}?view=week&event=${DENTIST.id}`, api: calendarApi(), due: DUE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("region", { name: "Week" })).toBeVisible();
    await expect((await canvas.findAllByText(DENTIST.title)).length).toBeGreaterThan(1);
  },
};

/** Week on a phone: the page does not scroll sideways, the grid scrolls inside itself. */
export const WeekPhone: Story = {
  args: { path: `${SCHEDULE}?view=week`, api: calendarApi(), due: DUE },
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
  args: { path: SCHEDULE, api: calendarApi(), due: DUE },
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
  args: { path: `${SCHEDULE}?event=${FLIGHT.id}`, api: calendarApi(), due: DUE },
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
  args: { path: `${SCHEDULE}?event=${DENTIST.id}`, api: calendarApi(), due: DUE },
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
  args: { path: `${SCHEDULE}?event=${DENTIST.id}`, api: calendarApi(), due: DUE },
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
  args: { path: `${SCHEDULE}?event=${REVIEW.id}`, api: calendarApi(), due: DUE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: REVIEW.title });
    await expect(
      within(sheet).getByText(/Changed in Work meanwhile; we kept theirs/u),
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

/** A task due today: Paid (E), Hold time (H), Later (S), Open task (↵), and no Remove. */
export const TaskSheet: Story = {
  args: { path: `${SCHEDULE}?event=due:${DUE[1]?.id ?? ""}`, api: calendarApi(), due: DUE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: /Pay · Costa Obras invoice/u });
    const inSheet = within(sheet);
    await expect(inSheet.getByRole("button", { name: /Paid/u })).toBeVisible();
    await expect(inSheet.getByRole("button", { name: /Hold time/u })).toBeVisible();
    await expect(inSheet.getByRole("button", { name: /Later/u })).toBeVisible();
    await expect(inSheet.getByRole("button", { name: /Open task/u })).toBeVisible();
    await expect(inSheet.queryByRole("button", { name: /Remove/u })).toBeNull();
  },
};

/** An overdue task: marked in the danger colour; Paid takes it off with an Undo. */
export const TaskSheetOverdue: Story = {
  args: { path: `${SCHEDULE}?event=due:${DUE[0]?.id ?? ""}`, api: calendarApi(), due: DUE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sheet = await canvas.findByRole("complementary", { name: /Pay · EDP bill/u });
    await userEvent.click(within(sheet).getByRole("button", { name: /Paid/u }));
    await expect(await canvas.findByText(/EDP bill marked paid/u)).toBeVisible();
  },
};

/** Month: each day's first events, "+N more"; a dot per event on a phone. */
export const MonthDesktop: Story = {
  args: { path: `${SCHEDULE}?view=month`, api: calendarApi(), due: DUE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("group", { name: "Month" })).toBeVisible();
  },
};

export const MonthPhone: Story = {
  args: { path: `${SCHEDULE}?view=month`, api: calendarApi(), due: DUE },
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
  args: { path: SCHEDULE, api: calendarApi([]) },
  play: async ({ canvasElement }) => {
    await expect(await within(canvasElement).findByText("Nothing on your calendar.")).toBeVisible();
  },
};

/** New event (N, then E): the form, with the day looked at. */
export const NewEvent: Story = {
  args: { path: SCHEDULE, api: calendarApi(), due: DUE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(DENTIST.title);
    await userEvent.click(canvas.getByRole("button", { name: /New event/u }));
    await expect(await within(document.body).findByRole("dialog")).toBeInTheDocument();
  },
};

/* What Scout noticed, in the reader. */

/** The inbox around the lease email, with its read in `read`. */
function reader(read: () => Record<string, MockRoute>) {
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
      ...scheduleApi([...WEEK_EVENTS, HANDOVER], { calendars: [CAMPBOOKS] }).routes,
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

const CALENDAR_ACCESS = {
  ...CONNECTION,
  capabilities: ["messages", "calendar"],
} as const;
const MAIL_ONLY = {
  ...ZOHO_CONNECTION,
  address: "ana@lopesimoveis.example",
  health: "ok",
  capabilities: ["messages"],
} as const;

function settingsApi() {
  return {
    ...scheduleApi().routes,
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
    await expect(await canvas.findByRole("button", { name: "Add calendar access" })).toBeVisible();
    const holidays = await canvas.findByRole("switch", { name: "Show Holidays in Portugal" });
    await expect(holidays).toHaveAttribute("aria-checked", "false");
    await userEvent.click(canvas.getByRole("switch", { name: "Scout adds Stays" }));
    await waitFor(() =>
      expect(canvas.getByRole("switch", { name: "Scout adds Stays" })).toHaveAttribute(
        "aria-checked",
        "false",
      ),
    );
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
