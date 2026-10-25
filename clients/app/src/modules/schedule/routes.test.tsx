import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { W } from "../../../test/demo/conversations";
import { CONNECTION, ZOHO_CONNECTION } from "../../../test/demo/identity";
import {
  CALENDARS,
  DENTIST,
  FLIGHT,
  HOLIDAYS,
  REVIEW,
  SETTINGS,
  SCHEDULE_NOW,
  TASKS,
  taskSources,
  WEEK_EVENTS,
  conversationId,
  sourceDetail,
} from "../../../test/demo/schedule";
import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../test/render";
import type { MailboxConsent } from "~/modules/connectors";
import type { TaskSource, UseTaskSources } from "~/modules/tasks";

import { buildScheduleRoutes } from "./index";

const S = `${W}/schedule`;

const consent = (start: MailboxConsent["start"] = vi.fn()): MailboxConsent => ({
  providers: ["google", "zoho"],
  providersFailed: false,
  start,
  starting: false,
  refusal: undefined,
  signInAgain: vi.fn(),
  wayInFate: () => "unchanged",
});

/** The emails the demo tasks came from, as the app's conversations module tells them. */
const useSources: UseTaskSources = (_workspaceId, ids) =>
  new Map(
    taskSources()
      .filter((mail) => ids.includes(mail.id))
      .map((mail): [string, TaskSource] => [
        mail.id,
        {
          name: mail.latest_from.name ?? "",
          subject: mail.subject,
          date: mail.last_message_at,
          href: `/w/${DUARTE.id}/inbox/${mail.stream}/${mail.id}`,
        },
      ]),
  );

function renderAt(
  path: string,
  { locale = "en", start }: { locale?: "en" | "pt-PT"; start?: MailboxConsent["start"] } = {},
) {
  const root = createRootRoute({ component: Outlet });
  const workspace = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: Outlet,
  });
  const router = createRouter({
    routeTree: root.addChildren([
      workspace.addChildren(
        buildScheduleRoutes(workspace, { useConsent: () => consent(start), useSources }),
      ),
    ]),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />, locale) };
}

/** The API of the week, with the answers a sheet and its actions need. */
function api(extra: Parameters<typeof mockApi>[0] = {}) {
  return mockApi({
    [`GET ${S}/settings`]: { body: SETTINGS },
    [`GET ${S}/calendars`]: { body: { calendars: CALENDARS } },
    [`GET ${S}/events`]: { body: { events: WEEK_EVENTS, truncated: false } },
    [`GET ${W}/tasks`]: { body: { tasks: [] } },
    [`GET ${W}/conversations/${conversationId(5)}`]: { body: sourceDetail(5) },
    [`GET ${W}/conversations/${conversationId(1)}`]: { body: sourceDetail(1) },
    ...extra,
  });
}

const PATH = `/w/${DUARTE.id}/schedule`;
/** Tuesday 27 October 2026, as the demo week has it. */
const WEEK = `${PATH}?view=agenda&date=2026-10-27`;

/** Pretends the screen is a phone (narrower than 48rem) or not; jsdom has no matchMedia. */
function screenIs(width: "phone" | "wide") {
  vi.stubGlobal(
    "matchMedia",
    (query: string): MediaQueryList =>
      ({
        matches: width === "phone" && query === "(width < 48rem)",
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }) as unknown as MediaQueryList,
  );
}

describe("the Calendar place", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("opens on Week on a wide screen, and keeps Agenda in the address when chosen", async () => {
    screenIs("wide");
    api();
    const { router } = renderAt(`${PATH}?date=2026-10-27`);

    expect(await screen.findByRole("region", { name: "Week" })).toBeVisible();
    expect(router.state.location.search).not.toHaveProperty("view");

    fireEvent.keyDown(document, { key: "1" });
    expect(await screen.findByRole("heading", { name: /Tuesday/u })).toBeVisible();
    expect(router.state.location.search).toMatchObject({ view: "agenda" });
  });

  it("opens on Agenda on a phone, and keeps Week in the address when chosen", async () => {
    screenIs("phone");
    api();
    const { router } = renderAt(`${PATH}?date=2026-10-27`);

    expect(await screen.findByRole("heading", { name: /Tuesday/u })).toBeVisible();
    expect(screen.queryByRole("region", { name: "Week" })).toBeNull();

    fireEvent.keyDown(document, { key: "2" });
    expect(await screen.findByRole("region", { name: "Week" })).toBeVisible();
    expect(router.state.location.search).toMatchObject({ view: "week" });
  });

  it("lets a view in the address win at either width", async () => {
    screenIs("phone");
    api();
    const first = renderAt(`${PATH}?view=week&date=2026-10-27`);
    expect(await screen.findByRole("region", { name: "Week" })).toBeVisible();
    first.unmount();

    screenIs("wide");
    renderAt(`${PATH}?view=agenda&date=2026-10-27`);
    expect(await screen.findByRole("heading", { name: /Tuesday/u })).toBeVisible();
    expect(screen.queryByRole("region", { name: "Week" })).toBeNull();
  });

  it("lists the days from the one looked at, skipping days with nothing", async () => {
    api();
    renderAt(WEEK);

    expect(await screen.findByRole("button", { name: /Dentist check-up/u })).toBeVisible();
    expect(screen.getByRole("heading", { name: /Tuesday/u })).toBeVisible();
    expect(screen.getByRole("heading", { name: /Friday/u })).toBeVisible();
    expect(screen.queryByRole("heading", { name: /Saturday/u })).toBeNull();
    expect(screen.queryByText(/Overdue/u)).toBeNull();
  });

  it("says so in one line when nothing is ahead", async () => {
    api({ [`GET ${S}/events`]: { body: { events: [], truncated: false } } });
    renderAt(WEEK);

    expect(await screen.findByText("Nothing on your calendar.")).toBeVisible();
  });

  it("opens an event's sheet, with the email it was read from, and closes it with Escape", async () => {
    api();
    const { router } = renderAt(WEEK);

    fireEvent.click(await screen.findByRole("button", { name: /Flight LIS → AMS/u }));

    const sheet = await screen.findByRole("complementary", { name: FLIGHT.title });
    expect(await within(sheet).findByText("Booking confirmed: Lisbon → Amsterdam")).toBeVisible();
    expect(within(sheet).getByRole("button", { name: /Add to my calendar/u })).toBeVisible();
    expect(within(sheet).getByRole("button", { name: /Edit/u })).toBeVisible();
    expect(within(sheet).getByRole("button", { name: /Remove/u })).toBeVisible();
    expect(router.state.location.search).toMatchObject({ event: FLIGHT.id });

    fireEvent.keyDown(document, { key: "Escape" });

    await waitFor(() => {
      expect(screen.queryByRole("complementary", { name: FLIGHT.title })).toBeNull();
    });
  });

  it("shows the time zone an event was written in when it is not the member's", async () => {
    api();
    renderAt(`${WEEK}&event=${FLIGHT.id}`);

    const sheet = await screen.findByRole("complementary", { name: FLIGHT.title });
    expect(within(sheet).getByText("08:40 – 11:25 Amsterdam time")).toBeVisible();
  });

  it("says a change was not kept, and answers an invitation", async () => {
    const calls = api({
      [`POST ${S}/events/${REVIEW.id}/response`]: {
        body: { event: { ...REVIEW, response: "accepted" }, previous_response: "needs_action" },
      },
    });
    renderAt(`${WEEK}&event=${REVIEW.id}`);

    const sheet = await screen.findByRole("complementary", { name: REVIEW.title });
    expect(within(sheet).getByText("Changed in Google meanwhile; we kept theirs")).toBeVisible();
    expect(within(sheet).getByText("Synced both ways with Google")).toBeVisible();
    fireEvent.click(within(sheet).getByRole("button", { name: "Maybe" }));

    await waitFor(() => {
      expect(calls.calls.find((call) => call.route.endsWith("/response"))?.body).toEqual({
        response: "tentative",
      });
    });
  });

  it("removes an event and offers to take it back", async () => {
    const calls = api({
      [`DELETE ${S}/events/${DENTIST.id}`]: { body: { id: DENTIST.id, undo_token: "token" } },
      [`POST ${S}/events/${DENTIST.id}/restore`]: { body: DENTIST },
    });
    renderAt(`${WEEK}&event=${DENTIST.id}`);

    const sheet = await screen.findByRole("complementary", { name: DENTIST.title });
    fireEvent.click(within(sheet).getByRole("button", { name: /Remove/u }));

    expect(
      await screen.findByText("Dentist check-up removed. Scout won’t add it again."),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));

    await waitFor(() => {
      expect(calls.calls.find((call) => call.route.endsWith("/restore"))?.body).toEqual({
        undo_token: "token",
      });
    });
  });

  it("adds an event to a calendar of the member's, last used first", async () => {
    const calls = api({
      [`POST ${S}/events/${DENTIST.id}/push`]: { body: { ...DENTIST, on_provider: true } },
    });
    renderAt(`${WEEK}&event=${DENTIST.id}`);

    const sheet = await screen.findByRole("complementary", { name: DENTIST.title });
    fireEvent.click(within(sheet).getByRole("button", { name: /Add to my calendar/u }));
    const choices = within(await screen.findByRole("dialog", { name: "Add to which calendar?" }));
    expect(choices.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Personal",
      "Work",
    ]);
    fireEvent.click(choices.getByRole("button", { name: "Work" }));

    await waitFor(() => {
      expect(calls.calls.find((call) => call.route.endsWith("/push"))?.body).toEqual({
        calendar_id: CALENDARS[2]?.id,
      });
    });
  });

  it("switches views with 1, 2 and 3", async () => {
    api();
    const { router } = renderAt(WEEK);
    await screen.findByRole("button", { name: /Dentist check-up/u });

    fireEvent.keyDown(document, { key: "2" });
    expect(await screen.findByRole("region", { name: "Week" })).toBeVisible();
    // Week is the default on a wide screen, so the address says nothing.
    expect(router.state.location.search).not.toHaveProperty("view");

    fireEvent.keyDown(document, { key: "3" });
    expect(await screen.findByRole("group", { name: "Month" })).toBeVisible();

    fireEvent.keyDown(document, { key: "1" });
    expect(await screen.findByRole("heading", { name: /Tuesday/u })).toBeVisible();
  });

  it("moves a week with J and K, and back to today with T", async () => {
    api();
    const { router } = renderAt(`${PATH}?view=week&date=2026-10-27`);
    await screen.findByRole("region", { name: "Week" });

    fireEvent.keyDown(document, { key: "j" });
    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({ date: "2026-11-03" });
    });
    fireEvent.keyDown(document, { key: "k" });
    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({ date: "2026-10-27" });
    });
    fireEvent.keyDown(document, { key: "t" });
    await waitFor(() => {
      expect(router.state.location.search).not.toHaveProperty("date");
    });
  });

  it("places the week's events by the minute, side by side where they overlap", async () => {
    api();
    renderAt(`${PATH}?view=week&date=2026-10-27`);

    const block = await screen.findByRole("button", { name: /Dentist check-up/u });
    // 15:00 on a grid that starts at 07:00, for 45 minutes, alone in its day.
    expect(block.getAttribute("style")).toContain("--start: 480");
    expect(block.getAttribute("style")).toContain("--len: 45");
    expect(block.getAttribute("style")).toContain("--columns: 1");
    // The hotel crosses days: it is in the All day lane, not on the grid.
    expect(
      screen.getByRole("button", { name: /Hotel Canal Verde/u }).getAttribute("style"),
    ).toContain("grid-column");
  });

  it("opens a new event form from the toolbar", async () => {
    api();
    renderAt(WEEK);

    fireEvent.click(await screen.findByRole("button", { name: "New event" }));

    expect(await screen.findByRole("dialog", { name: "New event" })).toBeVisible();
  });

  it("reads days and times in the member's language", async () => {
    api();
    renderAt(WEEK, { locale: "pt-PT" });

    expect(await screen.findByRole("heading", { name: /terça-feira/u })).toBeVisible();
    expect(screen.getByText(/Hora de Lisbon/u)).toBeVisible();
    expect(screen.getByRole("button", { name: "Mês" })).toBeVisible();
  });
});

describe("tasks on the calendar", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /** Ana's open tasks, answering Done by taking the task off the list, as the server would. */
  function withTasks(view = WEEK) {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(SCHEDULE_NOW));
    let open = [...TASKS];
    const mocked = api({
      [`GET ${W}/tasks`]: () => ({ body: { tasks: open } }),
      ...Object.fromEntries(
        TASKS.map((each) => [
          `POST ${W}/tasks/${each.id}/done`,
          () => {
            open = open.filter((other) => other.id !== each.id);
            return { body: { ...each, state: "done", revision: 2 } };
          },
        ]),
      ),
    });
    return { mocked, view: renderAt(view) };
  }

  it("shows tasks on the day they are due, an overdue one on today in the danger tone", async () => {
    withTasks();

    expect((await screen.findAllByText("Overdue · 2 days")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /Pay · Costa Obras invoice/u })).toBeVisible();
    expect(screen.getByRole("button", { name: /Do · Send the signed lease/u })).toBeVisible();
  });

  it("shows a task on its planned day as well, and a slipped plan on today", async () => {
    withTasks();

    // The lease: planned Wednesday, due Thursday, two rows.
    expect(
      await screen.findAllByRole("button", { name: /Do · Send the signed lease/u }),
    ).toHaveLength(2);
    expect(screen.getByText("Planned Mon")).toBeVisible();
  });

  it("shows one row, saying it is planned, when it is planned for its due day", async () => {
    withTasks();

    expect(
      await screen.findAllByRole("button", { name: /Pay · Costa Obras invoice/u }),
    ).toHaveLength(1);
  });

  it("shows no Planned row for a task put off until later, and keeps its Due row", async () => {
    withTasks();

    expect(
      await screen.findAllByRole("button", { name: /Do · Book the car inspection/u }),
    ).toHaveLength(1);
  });

  it("opens a task's sheet with Paid and Open task, and no Hold time, Later or Remove", async () => {
    withTasks();

    fireEvent.click(await screen.findByRole("button", { name: /Pay · EDP bill/u }));

    const sheet = await screen.findByRole("complementary", { name: /Pay · EDP bill/u });
    for (const name of [/Paid/u, /Open task/u]) {
      expect(within(sheet).getByRole("button", { name })).toBeVisible();
    }
    for (const name of [/Hold time/u, /Later/u, /Remove/u]) {
      expect(within(sheet).queryByRole("button", { name })).toBeNull();
    }
    expect(within(sheet).getByText(/^Was due .*25/u)).toBeVisible();
    expect(within(sheet).getByText("Your October bill")).toBeVisible();
  });

  it("tells a Planned row's sheet apart: the Planned chip, the plan and the due day", async () => {
    withTasks(`${PATH}?view=agenda&date=2026-10-28`);

    fireEvent.click(
      await screen.findByRole("button", { name: /^(?=.*Send the signed lease)(?=.*Planned)/u }),
    );

    const sheet = await screen.findByRole("complementary", { name: /Do · Send the signed lease/u });
    expect(within(sheet).getByText("Planned")).toBeVisible();
    expect(within(sheet).getByText(/^Planned for .*28/u)).toBeVisible();
    expect(within(sheet).getByText(/^Due .*29/u)).toBeVisible();
  });

  it("marks it paid with E, takes it off the calendar, and offers Undo", async () => {
    const { mocked } = withTasks();

    fireEvent.click(await screen.findByRole("button", { name: /Pay · EDP bill/u }));
    await screen.findByRole("complementary", { name: /Pay · EDP bill/u });
    fireEvent.keyDown(document, { key: "e" });

    expect(await screen.findByText("Paid · EDP bill")).toBeVisible();
    await waitFor(() => {
      expect(mocked.calls.map((call) => call.route)).toContain(
        `POST ${W}/tasks/${TASKS[0]?.id ?? ""}/done`,
      );
    });
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: /Pay · EDP bill/u })).toBeNull();
    });
  });

  it("shows no task row where there are no tasks", async () => {
    api();
    renderAt(WEEK);

    await screen.findByRole("button", { name: /Dentist check-up/u });
    expect(screen.queryByText(/Pay ·/u)).toBeNull();
  });
});

describe("Settings → Calendar", () => {
  const connections = [
    { ...CONNECTION, capabilities: ["messages", "calendar"] },
    {
      ...ZOHO_CONNECTION,
      address: "ana@lopesimoveis.example",
      health: "ok",
      capabilities: ["messages"],
    },
    {
      ...CONNECTION,
      id: "0192f1a0-0000-7000-8000-0000000000c9",
      address: "ana.work@example.com",
      capabilities: ["messages"],
    },
  ];

  function settings(extra: Parameters<typeof mockApi>[0] = {}) {
    return api({
      [`GET ${W}/connections`]: { body: { connections } },
      ...extra,
    });
  }

  it("lists the calendars and turns one on", async () => {
    const calls = settings({
      [`PATCH ${S}/calendars/${HOLIDAYS.id}`]: { body: { ...HOLIDAYS, syncing: true } },
    });
    renderAt(`/w/${DUARTE.id}/settings/calendar`);

    expect(await screen.findByRole("heading", { name: "Calendars" })).toBeVisible();
    expect(await screen.findByText("Always on")).toBeVisible();
    fireEvent.click(await screen.findByRole("switch", { name: "Show Holidays in Portugal" }));
    await waitFor(() => {
      expect(
        calls.calls.find((call) => call.route === `PATCH ${S}/calendars/${HOLIDAYS.id}`)?.body,
      ).toEqual({ syncing: true });
    });
  });

  it("offers calendar access to a Google mailbox without it, and starts its consent", async () => {
    const start = vi.fn();
    settings();
    renderAt(`/w/${DUARTE.id}/settings/calendar`, { start });

    // Only the Google mailbox without calendar access: not the one that has it, not Zoho.
    const add = await screen.findByRole("button", { name: "Add calendar access" });
    expect(screen.getAllByRole("button", { name: "Add calendar access" })).toHaveLength(1);
    fireEvent.click(add);

    expect(start).toHaveBeenCalledWith({
      purpose: "reconnect",
      connection: "0192f1a0-0000-7000-8000-0000000000c9",
    });
  });

  it("turns off a kind Scout adds, keeping the others", async () => {
    const calls = settings({
      [`PATCH ${S}/settings`]: {
        body: {
          ...SETTINGS,
          scout_kinds: SETTINGS.scout_kinds.filter((kind) => kind !== "delivery"),
        },
      },
    });
    renderAt(`/w/${DUARTE.id}/settings/calendar`);

    fireEvent.click(await screen.findByRole("switch", { name: "Scout adds Deliveries" }));

    await waitFor(() => {
      expect(calls.calls.find((call) => call.route === `PATCH ${S}/settings`)?.body).toEqual({
        scout_kinds: SETTINGS.scout_kinds.filter((kind) => kind !== "delivery"),
      });
    });
  });

  it("sets where else a kind goes, replacing the whole map", async () => {
    const calls = settings({ [`PATCH ${S}/settings`]: { body: SETTINGS } });
    renderAt(`/w/${DUARTE.id}/settings/calendar`);

    // The kinds are listed in order: travel, stay, show…
    const selects = await screen.findAllByRole("combobox", { name: "Also add to" });
    fireEvent.change(selects[2] ?? document.body, { target: { value: CALENDARS[2]?.id } });

    await waitFor(() => {
      expect(calls.calls.find((call) => call.route === `PATCH ${S}/settings`)?.body).toMatchObject({
        also_add_to: { show: CALENDARS[2]?.id },
      });
    });
  });
});
