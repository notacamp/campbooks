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
  JONAS,
  PEDRO,
  PHISH,
  PHISH_DETAIL,
  SOFIA,
} from "../../../../../test/demo/attention";
import { NOW, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

function renderHome() {
  const root = createRootRoute({ component: Outlet });
  const home = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () => <WhatNeedsYou workspaceId={DUARTE.id} now={NOW} />,
  });
  const conversation = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/inbox/$stream/$conversationId",
    validateSearch: (search: Record<string, unknown>) =>
      search["reply"] === true ? { reply: true } : {},
    component: () => <p>Reader</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, conversation]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

function api(items = ITEMS, live?: { items: typeof ITEMS }) {
  return mockApi({
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: live?.items ?? items, counts: COUNTS } },
    [`GET ${W}/conversations`]: { body: { conversations: [...NEEDS, SOFIA], next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/conversations/${PHISH.id}`]: { body: PHISH_DETAIL },
    [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: { body: { ...PHISH, need: null } },
    [`POST ${W}/conversations/${PHISH.id}/need/reopen`]: { body: PHISH },
    [`POST ${W}/conversations/${MARTA.id}/need/resolve`]: { body: { ...MARTA, need: null } },
    [`POST ${W}/conversations/${JONAS.id}/need/resolve`]: { body: { ...JONAS, need: null } },
    [`POST ${W}/attention/${ITEMS[3]?.id ?? ""}/snooze`]: { body: ITEMS[3] },
  });
}

/** The row (button) holding `text`, and whether it is the selected one. */
function isSelected(text: string): boolean {
  return screen.getByText(text).closest("button")?.hasAttribute("data-selected") === true;
}

describe("What needs you", () => {
  it("groups what needs you, Review first, in plain words", async () => {
    api();
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeVisible();
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 4)).toEqual(["Review 1", "Reply 2", "Do 1", "Pay 2"]);
    expect(screen.getByText("Marta Ferreira is waiting for your reply")).toBeVisible();
    expect(screen.getByText("Due tomorrow")).toBeVisible();
    // The warning names the sender by address; no percentage anywhere.
    expect(screen.getByText("it-support@at3lier-sol.example")).toBeVisible();
    expect(document.body).not.toHaveTextContent("%");
    expect(screen.getByText("Replied · Tile samples")).toBeVisible();
  });

  it("keeps the sender's words as plain text", async () => {
    api();
    renderHome();
    const quote = await screen.findByText(
      "Could you confirm Thursday’s walkthrough? The builders need to know by Wednesday.",
    );
    expect(quote.tagName).toBe("Q");
    expect(screen.queryAllByRole("link")).toHaveLength(0);
  });

  it("marks the warning safe with X and takes it back with Z", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "x" });
    await waitFor(() => {
      expect(screen.queryByText("This may be phishing. Don’t use its links.")).toBeNull();
    });
    // It names the need it acted on: the server refuses one that changed.
    expect(calls.calls.find((c) => c.route.endsWith("/need/resolve"))?.body).toEqual({
      outcome: "not_needed",
      message_id: PHISH.need?.message_id,
    });
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${PHISH.id}/need/reopen`,
      );
    });
    // Back on the list at once.
    expect(await screen.findByText("This may be phishing. Don’t use its links.")).toBeVisible();
  });

  it("opens an item with Enter: its tells and how Campbooks decided", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "Enter" });
    expect(
      await screen.findByText(
        "The sender’s domain looks like a better-known one (at3lier-sol.example).",
      ),
    ).toBeVisible();
    expect(screen.getByText("How Campbooks decided")).toBeVisible();
  });

  it("moves with the arrow keys, which scroll again while an item is open", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "ArrowDown" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    fireEvent.keyDown(document, { key: "ArrowUp" });
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    fireEvent.keyDown(document, { key: "Enter" });
    await screen.findByText("How Campbooks decided");
    // Open: ↓ is the browser's (no step), J still steps.
    expect(fireEvent.keyDown(document, { key: "ArrowDown" })).toBe(true);
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
  });

  it("replies with R in the conversation's reader", async () => {
    api();
    const { router } = renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "r" });
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/inbox/primary/${MARTA.id}`);
    });
    expect(router.state.location.search).toEqual({ reply: true });
  });

  it("puts an item off until Monday with L", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("Pedro Alves asked you to do something");
    for (const key of ["j", "j", "j"]) {
      fireEvent.keyDown(document, { key });
    }
    fireEvent.keyDown(document, { key: "l" });
    const menu = await screen.findByRole("dialog", { name: "Bring it back" });
    fireEvent.click(within(menu).getByRole("button", { name: "Monday" }));
    await waitFor(() => {
      expect(screen.queryByText("Pedro Alves asked you to do something")).toBeNull();
    });
    const snooze = calls.calls.find((c) => c.route.endsWith("/snooze"));
    expect(new Date((snooze?.body as { until: string }).until).getDay()).toBe(1);
    expect(PEDRO.need?.reason).toBe("do");
  });

  it("keeps the selection when a live update takes the selected item away", async () => {
    const live = { items: ITEMS };
    const calls = api(ITEMS, live);
    const { queryClient } = renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    // The first item is selected from the start; J moves to Marta.
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    // Marta is replied to elsewhere: the live update drops her.
    live.items = ITEMS.filter((item) => item.record.id !== MARTA.id);
    await queryClient.invalidateQueries();
    await waitFor(() => {
      expect(screen.queryByText("Marta Ferreira is waiting for your reply")).toBeNull();
    });
    // Her neighbour takes the selection, not the top of the list.
    expect(isSelected("Jonas Weber asked you a question")).toBe(true);
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${JONAS.id}/need/resolve`,
      );
    });
    expect(calls.calls.map((c) => c.route)).not.toContain(
      `POST ${W}/conversations/${PHISH.id}/need/resolve`,
    );
  });

  it("puts the item back and says so when the need changed meanwhile", async () => {
    mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
      [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: problem(409, "need_changed"),
    });
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "e" });
    expect(await screen.findByText(/changed while you looked/u)).toBeVisible();
    expect(screen.getByText("This may be phishing. Don’t use its links.")).toBeVisible();
  });

  it("says when nothing needs you", async () => {
    api([]);
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "Nothing needs you" }),
    ).toBeVisible();
  });

  it("explains when it cannot load", async () => {
    mockApi({
      [`GET ${W}/attention`]: problem(503, "unavailable"),
      [`GET ${W}/conversations`]: { body: { conversations: [], next_cursor: null } },
    });
    renderHome();
    expect(await screen.findByText("Could not load what needs you")).toBeVisible();
  });
});
