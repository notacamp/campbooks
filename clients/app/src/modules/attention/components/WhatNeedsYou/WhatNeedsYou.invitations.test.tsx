// Invitations awaiting an answer on Home (campbooks-cloud#360): Decide rows
// in their band by the day of the event, with who asked, when and where,
// whether the member is free then, and Accept, Decline and Maybe on 1, 2 and
// 3 with Undo. The schedule module reads and answers them; Home places them.
// The demo's 15:00 UTC is 11:00 on Sam's New York calendar.
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  CLEARED,
  COUNTS,
  ITEMS,
  MARTA,
  MARTA_DETAIL,
  NEEDS,
} from "../../../../../test/demo/attention";
import { NOW, W } from "../../../../../test/demo/conversations";
import {
  HOME_CALENDARS,
  HOME_DUE,
  HOME_EVENTS,
  HOME_INVITATION_EVENTS,
  HOME_OFFSITE,
  HOME_OFFSITE_ITEM,
  HOME_SETTINGS,
  HOME_WEBINAR,
  HOME_WEBINAR_ITEM,
} from "../../../../../test/demo/home";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

function renderHome() {
  const root = createRootRoute({ component: Outlet });
  const home = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () => <WhatNeedsYou workspaceId={DUARTE.id} now={NOW} />,
  });
  const calendar = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/schedule",
    validateSearch: (search: Record<string, unknown>) => search,
    component: () => <p>Calendar</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, calendar]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

const S = `${W}/schedule`;
const INVITES = [HOME_WEBINAR_ITEM, HOME_OFFSITE_ITEM];

function api({ calendars = HOME_CALENDARS } = {}) {
  let answer: string | null = "needs_action";
  return mockApi({
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : {
            body: {
              items:
                answer === "needs_action" ? [...ITEMS, ...INVITES] : [...ITEMS, HOME_WEBINAR_ITEM],
              counts: COUNTS,
            },
          },
    [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/tasks`]: { body: { tasks: [HOME_DUE] } },
    [`GET ${S}/settings`]: { body: HOME_SETTINGS },
    [`GET ${S}/calendars`]: { body: { calendars } },
    [`GET ${S}/events`]: () => ({
      body: {
        events: [...HOME_EVENTS, ...HOME_INVITATION_EVENTS].map((each) =>
          each.id === HOME_OFFSITE.id ? { ...each, response: answer } : each,
        ),
        truncated: false,
      },
    }),
    [`POST ${S}/events/${HOME_OFFSITE.id}/response`]: (_request, body) => {
      const before = answer ?? "needs_action";
      answer = (body as { response: string }).response;
      return {
        body: {
          event: { ...HOME_OFFSITE, response: answer, pending: true },
          previous_response: before,
        },
      };
    },
  });
}

/** The row (button) holding `text`. */
function rowOf(text: string): HTMLElement {
  const row = screen.getByText(text).closest("button");
  if (row === null) {
    throw new Error(`no row holds ${text}`);
  }
  return row;
}

describe("Invitations on Home", () => {
  it("lists each invitation as a Decide row, counted, by the day of its event", async () => {
    api();
    renderHome();
    await screen.findByText("Invitation: Offsite planning · Office, 3rd floor");
    expect(screen.getByRole("heading", { level: 1, name: "8 things need you" })).toBeVisible();

    const offsite = rowOf("Invitation: Offsite planning · Office, 3rd floor");
    expect(offsite.textContent).toMatch(/Decide.*Event Thu.*Beatriz Lemos/u);
    expect(offsite.textContent).toContain("14:00 – 15:00");
    expect(within(offsite).getByText("Free then")).toBeVisible();
    const week = screen.getByRole("heading", { level: 2, name: /^This week/u }).closest("section");
    expect(
      within(week ?? document.body).getByText("Invitation: Offsite planning · Office, 3rd floor"),
    ).toBeVisible();

    // From a service, tomorrow, while the furniture comes.
    const webinar = rowOf("Invitation: Cash flow basics webinar · webinars.ledgerly.example");
    expect(webinar.textContent).toMatch(/Event tomorrow.*Ledgerly Webinars/u);
    expect(within(webinar).getByText("Busy then")).toBeVisible();
    expect(within(webinar).getByText("Furniture delivery, 2 of 3 parcels")).toBeVisible();
    const today = screen.getByRole("heading", { level: 2, name: /^Today/u }).closest("section");
    expect(
      within(today ?? document.body).getByText(
        "Invitation: Cash flow basics webinar · webinars.ledgerly.example",
      ),
    ).toBeVisible();
  });

  it("never shows an invitation again as a calendar row", async () => {
    api();
    renderHome();
    const coming = await screen.findByRole("heading", { level: 2, name: "Coming up" });
    const section = coming.closest("section") ?? document.body;
    await within(section).findByText("Furniture delivery, 2 of 3 parcels");
    expect(within(section).queryByText(HOME_WEBINAR.title)).toBeNull();
    expect(within(section).queryByText(HOME_OFFSITE.title)).toBeNull();
  });

  it("answers on 1, 2 and 3; 1 accepts, with Undo in the toast", async () => {
    const mocked = api();
    renderHome();
    fireEvent.click(await screen.findByText("Invitation: Offsite planning · Office, 3rd floor"));
    const actions = screen.getByRole("group", { name: "Actions for Offsite planning" });
    expect(within(actions).getByRole("button", { name: /^Accept/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "1",
    );
    expect(within(actions).getByRole("button", { name: /^Decline/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "2",
    );
    expect(within(actions).getByRole("button", { name: /^Maybe/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "3",
    );
    expect(within(actions).getByRole("button", { name: /Open in Calendar/u })).toBeVisible();

    fireEvent.keyDown(document, { key: "1" });
    await waitFor(() => {
      expect(
        mocked.calls.find((call) => call.route === `POST ${S}/events/${HOME_OFFSITE.id}/response`)
          ?.body,
      ).toEqual({ response: "accepted" });
    });
    // The row's own confirmation, next to the toast saying the same.
    expect((await screen.findAllByText("Accepted: Offsite planning")).length).toBeGreaterThan(0);
    await waitFor(() => {
      expect(screen.queryByText("Invitation: Offsite planning · Office, 3rd floor")).toBeNull();
    });

    // Z takes the latest back.
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(
        mocked.calls
          .filter((call) => call.route === `POST ${S}/events/${HOME_OFFSITE.id}/response`)
          .map((call) => call.body),
      ).toEqual([{ response: "accepted" }, { response: "needs_action" }]);
    });
    expect(
      await screen.findByText("Invitation: Offsite planning · Office, 3rd floor"),
    ).toBeVisible();
  });

  it("opens the invitation in Calendar on its day with Enter", async () => {
    api();
    const { router } = renderHome();
    fireEvent.click(await screen.findByText("Invitation: Offsite planning · Office, 3rd floor"));
    fireEvent.keyDown(document, { key: "Enter" });
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/schedule`);
    });
    expect(router.state.location.search).toEqual({ date: "2026-10-15", event: HOME_OFFSITE.id });
  });

  it("offers no answer on a calendar that cannot be written", async () => {
    api({ calendars: HOME_CALENDARS.map((calendar) => ({ ...calendar, writable: false })) });
    renderHome();
    fireEvent.click(await screen.findByText("Invitation: Offsite planning · Office, 3rd floor"));
    const actions = screen.getByRole("group", { name: "Actions for Offsite planning" });
    expect(within(actions).queryByRole("button", { name: /^Accept/u })).toBeNull();
    expect(within(actions).getByRole("button", { name: /^Snooze/u })).toBeVisible();
    expect(within(actions).getByRole("button", { name: /Open in Calendar/u })).toBeVisible();
  });
});
