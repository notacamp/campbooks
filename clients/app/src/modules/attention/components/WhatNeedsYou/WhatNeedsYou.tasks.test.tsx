// Tasks on Home (campbooks-cloud#43): the user's open tasks for today sit in
// their bands beside conversations (campbooks-cloud#358), read through
// `tasks.list?ids=`, with Done (Paid), Later and Not needed.
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { COUNTS, MARTA } from "../../../../../test/demo/attention";
import { NOW, W } from "../../../../../test/demo/conversations";
import { BILL, LEASE, taskItem } from "../../../../../test/demo/tasks";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

/** The bill two days late (Now), then the lease due today (Today). */
const ITEMS = [
  taskItem(BILL, { band: "now", band_reason: { rule: "overdue", days: 2 } }),
  taskItem(LEASE, { band: "today", band_reason: { rule: "due_today" } }),
];

function renderHome() {
  const root = createRootRoute({ component: Outlet });
  const home = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () => <WhatNeedsYou workspaceId={DUARTE.id} now={NOW} />,
  });
  const task = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/tasks/$taskId",
    component: () => <p>Task</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, task]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

function api() {
  let items = ITEMS;
  return mockApi({
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "open"
        ? { body: { items, counts: COUNTS, tasks_elsewhere: 4 } }
        : { body: { items: [], counts: [] } },
    [`GET ${W}/conversations`]: { body: { conversations: [MARTA], next_cursor: null } },
    [`GET ${W}/tasks`]: { body: { tasks: [LEASE, BILL] } },
    [`POST ${W}/tasks/${LEASE.id}/done`]: () => {
      items = ITEMS.filter((item) => item.record.id !== LEASE.id);
      return { body: { ...LEASE, state: "done", revision: 2 } };
    },
    [`POST ${W}/tasks/${LEASE.id}/reopen`]: () => {
      items = ITEMS;
      return { body: { ...LEASE, revision: 3 } };
    },
    [`POST ${W}/tasks/${BILL.id}/plan`]: { body: { ...BILL, plan: "later", revision: 2 } },
    [`POST ${W}/tasks/${BILL.id}/not_needed`]: {
      body: { ...BILL, state: "not_needed", revision: 2 },
    },
  });
}

describe("Tasks on Home", () => {
  it("lists the late bill in Now and the lease in Today, saying what and why first", async () => {
    api();
    renderHome();
    expect(await screen.findByText("Send the signed lease to Marta")).toBeVisible();
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 2)).toEqual(["Now 1", "Today 1"]);
    expect(screen.getByText("Overdue · 2 days")).toBeVisible();
    expect(screen.getByText("Pay task", { exact: false })).toBeVisible();
    expect(screen.getByText("Due today")).toBeVisible();
    expect(screen.getByText("From Marta Ferreira · email")).toBeVisible();
    expect(screen.getByText("You wrote this")).toBeVisible();
    expect(screen.getByText("EDP electricity bill · €84.20")).toBeVisible();
  });

  it("marks the selected task done with E, and Z brings it back", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("Send the signed lease to Marta");
    // The lease is second: move onto it.
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(screen.queryByText("Send the signed lease to Marta")).toBeNull();
    });
    expect(calls.calls.find((c) => c.route.endsWith(`/tasks/${LEASE.id}/done`))?.body).toEqual({
      revision: 1,
    });
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith(`/tasks/${LEASE.id}/reopen`))?.body).toEqual({
        revision: 2,
      });
    });
    expect(await screen.findByText("Send the signed lease to Marta")).toBeVisible();
  });

  it("marks a task not needed with X", async () => {
    const calls = api();
    renderHome();
    // The bill is first, selected from the start.
    await screen.findByText("EDP electricity bill · €84.20");
    fireEvent.keyDown(document, { key: "x" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(`POST ${W}/tasks/${BILL.id}/not_needed`);
    });
  });

  it("says how many more tasks are in Tasks, with the way there", async () => {
    api();
    renderHome();
    expect(await screen.findByText("4 more in Tasks")).toBeVisible();
    expect(screen.getByRole("link", { name: /^Open Tasks/u })).toHaveAttribute(
      "href",
      `/w/${DUARTE.id}/tasks`,
    );
  });
});
