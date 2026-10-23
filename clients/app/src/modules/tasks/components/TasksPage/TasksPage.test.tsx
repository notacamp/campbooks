import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
  useParams,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MARTA } from "../../../../../test/demo/attention";
import { W } from "../../../../../test/demo/conversations";
import {
  ACCOUNTANT,
  BILL,
  INSPECTION,
  LEASE,
  OPEN_TASKS,
  PHOTOS,
  PROPOSAL,
  TASKS_NOW,
  TEAM,
  VENUE,
} from "../../../../../test/demo/tasks";
import { ANA, DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import type { TaskSource } from "../../source";

import { TasksPage } from "./TasksPage";

const SOURCES = new Map<string, TaskSource>([
  [
    MARTA.id,
    {
      name: "Marta Ferreira",
      subject: MARTA.subject,
      date: MARTA.last_message_at,
      href: `/w/${DUARTE.id}/inbox/primary/${MARTA.id}`,
    },
  ],
]);

function Route() {
  const { taskId } = useParams({ strict: false });
  return (
    <TasksPage workspaceId={DUARTE.id} taskId={taskId} useSources={() => SOURCES} now={TASKS_NOW} />
  );
}

function renderTasks(path = "tasks") {
  const root = createRootRoute({ component: Outlet });
  const list = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/tasks",
    component: Route,
  });
  const open = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/tasks/$taskId",
    component: Route,
  });
  const reader = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/inbox/$stream/$conversationId",
    component: () => <p>Reader</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([list, open, reader]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/${path}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

/** The button that opens the task titled `title` (not its check). */
const rowOf = (title: string) =>
  screen.findByRole("button", { name: new RegExp(`^(Do|Pay)\\s+${title}`, "u") });

/** The tasks API over a list the calls below change, as the server would. */
function api(start = OPEN_TASKS, handedOff = [VENUE]) {
  let tasks = [...start];
  return mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${W}/tasks`]: (request) =>
      new URL(request.url).searchParams.get("whose") === "handed_off"
        ? { body: { tasks: handedOff } }
        : { body: { tasks } },
    [`POST ${W}/tasks`]: (_request, body) => {
      const made = {
        ...PHOTOS,
        id: "0192f1a0-0000-7000-8000-0000000007ff",
        title: (body as { title: string }).title,
      };
      tasks = [...tasks, made];
      return { status: 201, body: made };
    },
    [`POST ${W}/tasks/${BILL.id}/done`]: () => {
      tasks = tasks.filter((task) => task.id !== BILL.id);
      return { body: { ...BILL, state: "done", revision: 2 } };
    },
    [`POST ${W}/tasks/${BILL.id}/reopen`]: () => {
      tasks = [BILL, ...tasks];
      return { body: { ...BILL, revision: 3 } };
    },
    [`POST ${W}/tasks/${LEASE.id}/plan`]: { body: { ...LEASE, plan: "later", revision: 2 } },
    [`POST ${W}/tasks/${VENUE.id}/take_back`]: {
      body: { ...VENUE, assignee: ANA.user_id, handed_by: null, revision: 2 },
    },
  });
}

describe("Tasks", () => {
  it("groups the open tasks by when they are due, with the Later fold closed", async () => {
    api();
    renderTasks();
    expect(await screen.findByRole("heading", { level: 1, name: "Tasks" })).toBeVisible();
    await screen.findByText("Send the signed lease to Marta");

    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups).toEqual(
      expect.arrayContaining(["Overdue 1", "Today 2", "Coming up 2", "No date yet 1", "Later 1"]),
    );
    // Where each came from, and the late bill in words.
    expect(screen.getByText("From Marta Ferreira · email")).toBeVisible();
    expect(screen.getAllByText("You wrote this").length).toBeGreaterThan(0);
    expect(screen.getByText("Overdue · 2 days")).toBeVisible();
    expect(screen.getByText(PROPOSAL.title)).toBeVisible();
    expect(screen.getByText(ACCOUNTANT.title)).toBeVisible();
    // Later is folded until opened.
    expect(screen.queryByText(INSPECTION.title)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /^Show/ }));
    expect(await screen.findByText(INSPECTION.title)).toBeVisible();
  });

  it("shows a task someone else holds with their name and no check", async () => {
    api();
    renderTasks();
    await screen.findByText(VENUE.title);
    expect(screen.getByText("With Rui Matos")).toBeVisible();
    expect(screen.queryByRole("button", { name: `Mark done: ${VENUE.title}` })).toBeNull();
    expect(screen.getByRole("button", { name: `Mark done: ${LEASE.title}` })).toBeVisible();
  });

  it("marks the selected task paid with E, and Z brings it back", async () => {
    const calls = api();
    renderTasks();
    await screen.findByText(BILL.title);
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(screen.queryByText(BILL.title)).toBeNull();
    });
    expect(calls.calls.find((c) => c.route.endsWith(`/tasks/${BILL.id}/done`))?.body).toEqual({
      revision: 1,
    });
    // The undo reopens it with the revision the server answered.
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith(`/tasks/${BILL.id}/reopen`))?.body).toEqual({
        revision: 2,
      });
    });
    expect(await screen.findByText(BILL.title)).toBeVisible();
  });

  it("adds a task with a due chip and keeps the field for the next one", async () => {
    const calls = api();
    renderTasks();
    const field = await screen.findByRole("textbox", { name: "Add a task" });
    fireEvent.change(field, { target: { value: "Renew the parking permit" } });
    fireEvent.click(await screen.findByRole("button", { name: "Tomorrow" }));
    fireEvent.submit(field.closest("form") ?? field);
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route === `POST ${W}/tasks`)?.body).toEqual({
        title: "Renew the parking permit",
        due_on: "2026-10-14",
      });
    });
    expect(field).toHaveValue("");
    expect(field).toHaveFocus();
    expect(await screen.findByText("Renew the parking permit")).toBeVisible();
  });

  it("puts a task off until tomorrow morning from its detail", async () => {
    const calls = api();
    renderTasks();
    fireEvent.click(await rowOf(LEASE.title));
    expect(await screen.findByText("Where it came from")).toBeVisible();
    expect(screen.getByText(/Walkthrough on Thursday/u)).toBeVisible();
    fireEvent.click(await screen.findByRole("button", { name: /^Later/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Tomorrow" }));
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith(`/tasks/${LEASE.id}/plan`))?.body).toEqual({
        revision: 1,
        plan: "later",
        later_until: new Date(2026, 9, 14, 9).toISOString(),
      });
    });
  });

  it("takes back a task handed to a teammate", async () => {
    const calls = api();
    renderTasks();
    fireEvent.click(await rowOf(VENUE.title));
    expect((await screen.findAllByText("With Rui Matos")).length).toBeGreaterThan(1);
    fireEvent.click(await screen.findByRole("button", { name: "Take it back" }));
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route.endsWith(`/tasks/${VENUE.id}/take_back`))?.body,
      ).toEqual({
        revision: 1,
      });
    });
  });

  it("says so when nothing is left, and offers to add a task", async () => {
    api([], []);
    renderTasks();
    expect(await screen.findByText("Nothing left to do")).toBeVisible();
    expect(screen.getByRole("button", { name: /Add a task/ })).toBeVisible();
  });
});
