// "You replied. Is this done?" on Home (campbooks-cloud#396): a task Scout
// made from an email, after the member replied in that thread. Done and Not
// yet on 1 and 2, the thread on O, and why Scout asks when it opens.
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

import { COUNTS, MARTA } from "../../../../../test/demo/attention";
import { NOW, W } from "../../../../../test/demo/conversations";
import { ASK_REPLIED, taskItem } from "../../../../../test/demo/tasks";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

const ITEMS = [taskItem(ASK_REPLIED, { band: "whenever", band_reason: { rule: "no_date" } })];

function renderHome() {
  const root = createRootRoute({ component: Outlet });
  const home = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () => <WhatNeedsYou workspaceId={DUARTE.id} now={NOW} />,
  });
  const reader = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/inbox/$stream/$conversationId",
    component: () => <p>Reader</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, reader]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

function api() {
  return mockApi({
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "open"
        ? { body: { items: ITEMS, counts: COUNTS, tasks_elsewhere: 0 } }
        : { body: { items: [], counts: [] } },
    [`GET ${W}/conversations`]: { body: { conversations: [MARTA], next_cursor: null } },
    [`GET ${W}/tasks`]: { body: { tasks: [ASK_REPLIED] } },
    [`POST ${W}/tasks/${ASK_REPLIED.id}/done`]: {
      body: { ...ASK_REPLIED, state: "done", asks_done: false, revision: 3 },
    },
    [`POST ${W}/tasks/${ASK_REPLIED.id}/reply_answered`]: {
      body: { ...ASK_REPLIED, asks_done: false, revision: 3 },
    },
    [`POST ${W}/tasks/${ASK_REPLIED.id}/reply_unanswered`]: {
      body: { ...ASK_REPLIED, revision: 4 },
    },
  });
}

describe("A task that asks Done? on Home", () => {
  it("says you replied and when, what the task is, and asks", async () => {
    api();
    renderHome();
    expect(await screen.findByText("Send the signed lease")).toBeVisible();
    expect(screen.getByText("You replied", { exact: false })).toBeVisible();
    expect(screen.getByText("Task · Do · from Marta Ferreira")).toBeVisible();
    expect(
      screen.getByText("You answered Marta Ferreira’s email. Is this task done?"),
    ).toBeVisible();
    // Done and Not yet on 1 and 2, then the thread and Scout; the rest wait.
    const actions = screen.getByRole("group", { name: /Actions for Send the signed lease/u });
    const labels = within(actions)
      .getAllByRole("button")
      .map((button) => button.textContent);
    expect(labels[0]).toMatch(/^Done/u);
    expect(labels[1]).toMatch(/^Not yet/u);
    expect(labels[2]).toMatch(/^Open the thread/u);
    expect(within(actions).getByRole("button", { name: /^Open the thread/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "O",
    );
    expect(within(actions).queryByRole("button", { name: /Not needed/u })).toBeNull();
  });

  it("opens to say why Scout asks, and that it did not read the reply", async () => {
    api();
    renderHome();
    fireEvent.click(await screen.findByText("Send the signed lease"));
    expect(await screen.findByText("Why Scout asks")).toBeVisible();
    expect(screen.getByText(/Scout didn’t read your reply\./u)).toBeVisible();
    expect(screen.getByText("Nothing changes until you answer.")).toBeVisible();
  });

  it("marks it done with 1, and Z brings it back", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("Send the signed lease");
    fireEvent.keyDown(document, { key: "1" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route.endsWith(`/tasks/${ASK_REPLIED.id}/done`))?.body,
      ).toEqual({
        revision: 2,
      });
    });
    expect(await screen.findByText(`Done · ${ASK_REPLIED.title}`)).toBeVisible();
  });

  it("answers Not yet with 2: it stays open and says so, with an Undo", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("Send the signed lease");
    fireEvent.keyDown(document, { key: "2" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route.endsWith(`/tasks/${ASK_REPLIED.id}/reply_answered`))?.body,
      ).toEqual({ revision: 2 });
    });
    expect(await screen.findByText(`Okay, still open · ${ASK_REPLIED.title}`)).toBeVisible();
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route.endsWith(`/tasks/${ASK_REPLIED.id}/reply_unanswered`))
          ?.body,
      ).toEqual({ revision: 3 });
    });
  });

  it("opens the thread it came from with O", async () => {
    const { router } = (() => {
      api();
      return renderHome();
    })();
    await screen.findByText("Send the signed lease");
    fireEvent.keyDown(document, { key: "o" });
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(
        `/w/${DUARTE.id}/inbox/${MARTA.stream}/${MARTA.id}`,
      );
    });
  });
});
