// Your calendar on Home (campbooks-cloud#359): Next up above Now, the rest
// of today at the end of Today, Coming up in the aside. The schedule module
// reads it (`useHomeDay`); Home only places it. The demo's 15:00 UTC is 11:00
// on Sam's New York calendar.
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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
  HOME_DELIVERY,
  HOME_DUE,
  HOME_EVENTS,
  HOME_SETTINGS,
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
  const reader = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/inbox/$stream/$conversationId",
    component: () => <p>Reader</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, calendar, reader]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

const S = `${W}/schedule`;

function api({ items = ITEMS, events = HOME_EVENTS } = {}) {
  let pushed = false;
  const mocked = mockApi({
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items, counts: COUNTS } },
    [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/tasks`]: { body: { tasks: [HOME_DUE] } },
    [`GET ${S}/settings`]: { body: HOME_SETTINGS },
    [`GET ${S}/calendars`]: { body: { calendars: HOME_CALENDARS } },
    [`GET ${S}/events`]: () => ({
      body: {
        events: events.map((each) =>
          pushed && each.id === HOME_DELIVERY.id ? { ...each, on_provider: true } : each,
        ),
        truncated: false,
      },
    }),
    [`POST ${S}/events/${HOME_DELIVERY.id}/push`]: () => {
      pushed = true;
      return { body: { ...HOME_DELIVERY, on_provider: true } };
    },
    [`DELETE ${S}/events/${HOME_DELIVERY.id}/push`]: () => {
      pushed = false;
      return {
        body: { event: HOME_DELIVERY, id: HOME_DELIVERY.id, calendar_id: HOME_CALENDARS[1]?.id },
      };
    },
  });
  return mocked;
}

/** The row (button) holding `text`, and whether it is the selected one. */
function isSelected(text: string): boolean {
  return screen.getByText(text).closest("button")?.hasAttribute("data-selected") === true;
}

const CALL = "Kitchen plan call with Marta Ferreira";

describe("Your calendar on Home", () => {
  it("puts Next up above Now and today's events at the end of Today, uncounted", async () => {
    api();
    renderHome();
    const call = await screen.findByText(CALL);
    expect(
      await screen.findByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeVisible();
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 5)).toEqual([
      "Next up",
      "Now 2",
      "Today 1",
      "This week 2",
      "Whenever 1",
    ]);

    // Next up: the kind and how soon, where (the address's host), its time.
    const next = call.closest("button");
    expect(next?.textContent).toMatch(
      /Meeting · In 20 min.*Kitchen plan call with Marta Ferreira.*11:20 – 11:50.*meet\.example\.com/u,
    );
    // Today: its item, then the rest of the day in order; the declined one is not there.
    const today = screen.getByRole("heading", { level: 2, name: "Today 1" }).closest("section");
    const rows = within(today ?? document.body)
      .getAllByRole("button")
      .map((button) => button.textContent);
    expect(rows[0]).toMatch(/Marta Ferreira is waiting for your reply/u);
    expect(rows.slice(1)).toEqual([
      expect.stringMatching(/^Reservation · 12:30.*Lunch with Rui/u),
      expect.stringMatching(/^Held · 15:00.*Send the signed lease to Marta/u),
      expect.stringMatching(/^Appointment · 18:00.*Key handover/u),
    ]);
    expect(screen.queryByText("Supplier demo")).toBeNull();
    // The hero says the day's facts, not a guess.
    expect(
      screen.getByText(
        "Next: Kitchen plan call with Marta Ferreira at 11:20. 3 more on your calendar today.",
      ),
    ).toBeVisible();
  });

  it("moves through the events with the keys, and Enter opens one in Calendar", async () => {
    api();
    const { router } = renderHome();
    fireEvent.click(await screen.findByText("Marta Ferreira is waiting for your reply"));
    await screen.findByText("Lunch with Rui");
    // Past Marta, Today's last item, onto lunch.
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("Lunch with Rui")).toBe(true);
    const actions = screen.getByRole("group", { name: "Actions for Lunch with Rui" });
    expect(within(actions).getByRole("button", { name: /Open in Calendar/u })).toBeVisible();

    fireEvent.keyDown(document, { key: "Enter" });
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/schedule`);
    });
    expect(router.state.location.search).toEqual({ event: HOME_EVENTS[1]?.id });
  });

  it("puts Next up first for the keys, and ↓ goes on to Now", async () => {
    api();
    renderHome();
    fireEvent.click(await screen.findByText(CALL));
    expect(isSelected(CALL)).toBe(true);
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    fireEvent.keyDown(document, { key: "k" });
    expect(isSelected(CALL)).toBe(true);
  });

  it("joins a call at its web address, on 1", async () => {
    api();
    const open = vi.spyOn(globalThis, "open").mockReturnValue(null);
    renderHome();
    fireEvent.click(await screen.findByText(CALL));
    const join = await screen.findByRole("button", { name: /^Join/u });
    expect(join).toHaveAttribute("aria-keyshortcuts", "1");
    fireEvent.keyDown(document, { key: "1" });
    expect(open).toHaveBeenCalledWith(
      "https://meet.example.com/kxr-4471-tmp",
      "_blank",
      "noopener,noreferrer",
    );
  });

  it("opens the email an event was read from", async () => {
    api();
    const { router } = renderHome();
    fireEvent.click(await screen.findByText("Key handover · Rua das Flores 12"));
    const actions = await screen.findByRole("group", {
      name: "Actions for Key handover · Rua das Flores 12",
    });
    // No web address: no Join. The email is on 1.
    expect(within(actions).queryByRole("button", { name: /^Join/u })).toBeNull();
    fireEvent.click(await within(actions).findByRole("button", { name: /Open email/u }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(
        `/w/${DUARTE.id}/inbox/${MARTA.stream}/${MARTA.id}`,
      );
    });
  });

  it("lists the week in Coming up, and adds what only you see to your calendar, with Undo", async () => {
    const mocked = api();
    renderHome();
    const coming = await screen.findByRole("heading", { level: 2, name: "Coming up" });
    const section = coming.closest("section") ?? document.body;
    await within(section).findByText("Furniture delivery, 2 of 3 parcels");
    expect(within(section).getByText("Wed 14")).toBeVisible();
    expect(within(section).getByText("Review Marta’s proposal draft")).toBeVisible();
    // Only the delivery and the train are only yours: a ring and the link.
    expect(within(section).getAllByRole("img", { name: /Only you see it/u })).toHaveLength(2);
    const add = within(section).getByRole("button", {
      name: "Add to my calendar: Furniture delivery, 2 of 3 parcels",
    });
    expect(within(section).getAllByRole("button", { name: /^Add to my calendar: /u })).toHaveLength(
      2,
    );

    fireEvent.click(add);
    await waitFor(() => {
      expect(
        mocked.calls.find((call) => call.route === `POST ${S}/events/${HOME_DELIVERY.id}/push`)
          ?.body,
      ).toEqual({ calendar_id: HOME_CALENDARS[1]?.id });
    });
    expect(await screen.findByText("Added to Work")).toBeVisible();
    await waitFor(() => {
      expect(
        within(section).queryByRole("button", {
          name: "Add to my calendar: Furniture delivery, 2 of 3 parcels",
        }),
      ).toBeNull();
    });
    // Coming up's way to the whole calendar, with its keys.
    expect(within(section).getByRole("link", { name: /Open Calendar/u })).toHaveAttribute(
      "href",
      `/w/${DUARTE.id}/schedule`,
    );
  });

  it("shows Today for its events alone, with no count", async () => {
    api({ items: ITEMS.slice(0, 2) });
    renderHome();
    await screen.findByText("Lunch with Rui");
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 3)).toEqual(["Next up", "Now 2", "Today"]);
    expect(screen.getByRole("heading", { level: 1, name: "2 things need you" })).toBeVisible();
  });

  it("looks as it did with nothing on the calendar, tasks due or not", async () => {
    const mocked = api({ events: [] });
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeVisible();
    await waitFor(() => {
      expect(mocked.calls.some((call) => call.route === `GET ${S}/events`)).toBe(true);
    });
    expect(screen.queryByRole("heading", { level: 2, name: "Coming up" })).toBeNull();
    expect(screen.queryByText("Review Marta’s proposal draft")).toBeNull();
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 4)).toEqual(["Now 2", "Today 1", "This week 2", "Whenever 1"]);
    expect(
      screen.getByText("Campbooks reads your mail and keeps here what asks something of you."),
    ).toBeVisible();
  });
});
