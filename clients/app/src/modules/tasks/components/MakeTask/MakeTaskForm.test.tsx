import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MARTA } from "../../../../../test/demo/attention";
import { W } from "../../../../../test/demo/conversations";
import { LEASE, TASKS_NOW } from "../../../../../test/demo/tasks";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { MakeTaskForm } from "./MakeTaskForm";

function renderForm(onClose = vi.fn()) {
  const root = createRootRoute({
    component: () => (
      <MakeTaskForm
        workspaceId={DUARTE.id}
        conversationId={MARTA.id}
        subject="Walkthrough on Thursday"
        senderAddress="marta@casa-lume.example"
        now={TASKS_NOW}
        onClose={onClose}
      />
    ),
  });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return renderWithProviders(<RouterProvider router={router} />);
}

describe("Make it a task", () => {
  it("starts from the subject and makes the task from the conversation", async () => {
    const calls = mockApi({
      [`POST ${W}/tasks`]: { status: 201, body: { ...LEASE, title: "Walkthrough on Thursday" } },
    });
    const onClose = vi.fn();
    renderForm(onClose);
    const title = await screen.findByRole("textbox", { name: "Title" });
    expect(title).toHaveValue("Walkthrough on Thursday");
    fireEvent.click(screen.getByRole("button", { name: "Tomorrow" }));
    fireEvent.click(screen.getByRole("button", { name: /^Add task/ }));
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route === `POST ${W}/tasks`)?.body).toEqual({
        title: "Walkthrough on Thursday",
        due_on: "2026-10-14",
        source: { type: "conversations.conversation", id: MARTA.id },
        source_sender: "marta@casa-lume.example",
      });
    });
    // The form closes; the toast says so, with Undo and Open.
    await waitFor(() => {
      expect(onClose).toHaveBeenCalledTimes(1);
    });
    expect((await screen.findAllByText("Added to your tasks")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Open" })).toBeVisible();
    expect(screen.getByRole("button", { name: /^Undo/ })).toBeVisible();
  });

  it("closes without making anything on Cancel", async () => {
    const calls = mockApi({});
    const onClose = vi.fn();
    renderForm(onClose);
    fireEvent.click(await screen.findByRole("button", { name: /^Cancel/ }));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(calls.calls).toEqual([]);
  });
});
