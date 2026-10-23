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
  DUE_ITEMS,
  FLIGHT,
  HOLIDAYS,
  REVIEW,
  SETTINGS,
  SCHEDULE_NOW,
  WEEK_EVENTS,
  conversationId,
  sourceDetail,
} from "../../../test/demo/schedule";
import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../test/render";
import type { MailboxConsent } from "~/modules/connectors";

import { DUE_ACTION_KEY, dueKey, type DueItem } from "./due";
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
      workspace.addChildren(buildScheduleRoutes(workspace, { useConsent: () => consent(start) })),
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
    [`GET ${W}/conversations/${conversationId(5)}`]: { body: sourceDetail(5) },
    [`GET ${W}/conversations/${conversationId(1)}`]: { body: sourceDetail(1) },
    ...extra,
  });
}

const PATH = `/w/${DUARTE.id}/schedule`;
/** Tuesday 27 October 2026, as the demo week has it. */
const WEEK = `${PATH}?date=2026-10-27`;

describe("the Calendar place", () => {
  afterEach(() => {
    vi.useRealTimers();
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
    expect(within(sheet).getByText("Changed in Work meanwhile; we kept theirs")).toBeVisible();
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
    expect(router.state.location.search).toMatchObject({ view: "week" });

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

describe("the Due layer", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  const items = DUE_ITEMS as unknown as DueItem[];

  function withDue() {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(SCHEDULE_NOW));
    api();
    const view = renderAt(WEEK);
    view.queryClient.setQueryData(dueKey(DUARTE.id), items);
    return view;
  }

  it("shows tasks due on their day, and an overdue one on today in the danger tone", async () => {
    withDue();

    expect((await screen.findAllByText("Overdue · 2 days")).length).toBeGreaterThan(0);
    expect(
      screen.getByRole("button", { name: /Pay · Costa Obras invoice · €1,240\.00/u }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: /Do · Send the signed lease/u })).toBeVisible();
  });

  it("opens a task's sheet with Paid, Hold time, Later and Open task, and no Remove", async () => {
    withDue();

    fireEvent.click(await screen.findByRole("button", { name: /Pay · EDP bill/u }));

    const sheet = await screen.findByRole("complementary", { name: /Pay · EDP bill/u });
    for (const name of [/Paid/u, /Hold time/u, /Later/u, /Open task/u]) {
      expect(within(sheet).getByRole("button", { name })).toBeVisible();
    }
    expect(within(sheet).queryByRole("button", { name: /Remove/u })).toBeNull();
    expect(within(sheet).getByText(/^Was due .*25/u)).toBeVisible();
  });

  it("marks it paid with E, and offers Undo", async () => {
    const view = withDue();
    const done = vi.fn<(input: unknown) => Promise<{ label: string; undo: () => void }>>(() =>
      Promise.resolve({ label: "EDP bill marked paid", undo: vi.fn() }),
    );
    view.queryClient.setMutationDefaults(DUE_ACTION_KEY, { mutationFn: done });

    fireEvent.click(await screen.findByRole("button", { name: /Pay · EDP bill/u }));
    await screen.findByRole("complementary", { name: /Pay · EDP bill/u });
    fireEvent.keyDown(document, { key: "e" });

    expect(await screen.findByText("EDP bill marked paid")).toBeVisible();
    expect(done.mock.calls[0]?.[0]).toEqual({ item: items[0], action: "done" });
  });

  it("shows no Due row where nothing is wired", async () => {
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
  ];

  function settings(extra: Parameters<typeof mockApi>[0] = {}) {
    return api({
      [`GET ${W}/connections`]: { body: { connections } },
      ...extra,
    });
  }

  it("lists the calendars, turns one on, and does not offer calendar access yet", async () => {
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

    expect(screen.queryByRole("button", { name: "Add calendar access" })).toBeNull();
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
