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
import { describe, expect, it, vi } from "vitest";

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
import { ScoutChatProvider, type OpenScoutChat } from "~/lib/scout-chat";

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

function renderTasks(path = "tasks", openChat?: OpenScoutChat) {
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
  const app = <RouterProvider router={router} />;
  return {
    router,
    ...renderWithProviders(
      openChat === undefined ? app : <ScoutChatProvider open={openChat}>{app}</ScoutChatProvider>,
    ),
  };
}

/** The button that opens the task titled `title` (not its check). */
const rowOf = (title: string) =>
  screen.findByRole("button", { name: new RegExp(String.raw`^(Do|Pay)\s+${title}`, "u") });

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
    [`POST ${W}/tasks/${LEASE.id}/plan`]: { body: { ...LEASE, plan: "date", revision: 2 } },
    [`PATCH ${W}/tasks/${LEASE.id}`]: (_request, body) => ({
      body: { ...LEASE, title: (body as { title: string }).title, revision: 2 },
    }),
    [`POST ${W}/tasks/${LEASE.id}/done`]: { body: { ...LEASE, state: "done", revision: 3 } },
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

  it("asks Scout about the open task, from its detail", async () => {
    api();
    const open = vi.fn();
    renderTasks("tasks", open);
    fireEvent.click(await rowOf(LEASE.title));
    fireEvent.click(await screen.findByRole("button", { name: /^Ask Scout/u }));

    // Pressed with no focus of its own: the focus has nowhere to return to.
    expect(open).toHaveBeenCalledWith({ type: "tasks.task", id: LEASE.id }, { from: null });
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

  it("takes back a task handed to a teammate from its row, and it stays as mine at once", async () => {
    const calls = api();
    renderTasks();
    await screen.findByText(VENUE.title);
    fireEvent.click(screen.getByRole("button", { name: "Take it back" }));
    // Before the server answers it is already in my list, with a check.
    expect(screen.getByRole("button", { name: `Mark done: ${VENUE.title}` })).toBeVisible();
    expect(screen.queryByText("With Rui Matos")).toBeNull();
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route.endsWith(`/tasks/${VENUE.id}/take_back`))?.body,
      ).toEqual({
        revision: 1,
      });
    });
  });

  it("sends the revision the last answer gave when two changes come quickly", async () => {
    const calls = api();
    renderTasks();
    fireEvent.click(await rowOf(LEASE.title));
    const title = await screen.findByRole("textbox", { name: "Task title" });
    fireEvent.change(title, { target: { value: "Send the lease today" } });
    fireEvent.blur(title);
    fireEvent.click(screen.getByRole("button", { name: /^Done/ }));
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith(`/tasks/${LEASE.id}/done`))?.body).toEqual({
        revision: 2,
      });
    });
  });

  it("retries an undo that failed as an undo, not as the change", async () => {
    let reopened = 0;
    const handlers = mockApi({
      [`GET ${W}/members`]: { body: { members: TEAM } },
      [`GET ${W}/tasks`]: (request) =>
        new URL(request.url).searchParams.get("whose") === "handed_off"
          ? { body: { tasks: [] } }
          : { body: { tasks: [BILL] } },
      [`POST ${W}/tasks/${BILL.id}/done`]: { body: { ...BILL, state: "done", revision: 2 } },
      [`POST ${W}/tasks/${BILL.id}/reopen`]: () => {
        reopened += 1;
        return reopened === 1
          ? { status: 500, body: { code: "internal" } }
          : { body: { ...BILL, revision: 3 } };
      },
    });
    renderTasks();
    await screen.findByText(BILL.title);
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(screen.queryByText(BILL.title)).toBeNull();
    });
    fireEvent.keyDown(document, { key: "z" });
    fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(handlers.calls.filter((c) => c.route.endsWith("/reopen"))).toHaveLength(2);
    });
    // The retry did not close it again.
    expect(handlers.calls.filter((c) => c.route.endsWith("/done"))).toHaveLength(1);
    expect(await screen.findByText(BILL.title)).toBeVisible();
  });

  it("settles a typed date when the field is left, never on the way, and not in the past", async () => {
    const calls = api();
    renderTasks();
    fireEvent.click(await rowOf(LEASE.title));
    fireEvent.click(await screen.findByRole("button", { name: "Pick a day" }));
    const date = await screen.findByLabelText("Date");
    // Typing the year passes through 0002-…: nothing is sent.
    fireEvent.change(date, { target: { value: "0002-10-20" } });
    expect(calls.calls.some((c) => c.route.endsWith("/plan"))).toBe(false);
    // A day before today is dropped on leaving.
    fireEvent.change(date, { target: { value: "2026-10-01" } });
    fireEvent.blur(date);
    expect(calls.calls.some((c) => c.route.endsWith("/plan"))).toBe(false);
    fireEvent.change(screen.getByLabelText("Date"), { target: { value: "2026-10-20" } });
    fireEvent.keyDown(screen.getByLabelText("Date"), { key: "Enter" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith(`/tasks/${LEASE.id}/plan`))?.body).toEqual({
        revision: 1,
        plan: "date",
        plan_on: "2026-10-20",
      });
    });
  });

  it("acts on the open task with E, not on the row the cursor is on", async () => {
    const calls = api();
    renderTasks();
    fireEvent.click(await rowOf(LEASE.title));
    await screen.findByText("Where it came from");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(calls.calls.some((c) => c.route.endsWith("/done"))).toBe(true);
    });
    expect(calls.calls.filter((c) => c.route.endsWith("/done")).map((c) => c.route)).toEqual([
      `POST ${W}/tasks/${LEASE.id}/done`,
    ]);
  });

  it("says so when nothing is left, and offers to add a task", async () => {
    api([], []);
    renderTasks();
    expect(await screen.findByText("Nothing left to do")).toBeVisible();
    expect(screen.getByRole("button", { name: /Add a task/ })).toBeVisible();
  });
});
