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

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, HOLIDAYS, IDEAS, PAGES, plainDoc, viewOfDoc } from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { ANA, DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders, type MockHandler } from "../../../../test/render";
import type { PageView } from "../types";

import { PagesPage } from "./PagesPage";

// The editor needs a real browser; the page around it is what is tested here.
vi.mock("../editor/PageEditor", () => ({
  PageEditor: ({ editable }: { editable: boolean }) => (
    <p>{editable ? "editor (editable)" : "editor (read-only)"}</p>
  ),
}));

const P = `${W}/pages`;

function Route() {
  const { pageId } = useParams({ strict: false });
  return <PagesPage workspaceId={DUARTE.id} pageId={pageId} />;
}

function renderOpen(id: string) {
  const root = createRootRoute({ component: Outlet });
  const list = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages",
    component: Route,
  });
  const open = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages/$pageId",
    component: Route,
  });
  const router = createRouter({
    routeTree: root.addChildren([list, open]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/pages/${id}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

const view = (over: Partial<PageView> = {}) =>
  viewOfDoc({ ...HANDBOOK, created_by: ANA.user_id }, plainDoc("Welcome"), {
    ancestors: [{ id: HOLIDAYS.id, title: "Holiday calendar", icon: null }],
    ...over,
  });

function api(page: PageView, extra: Record<string, MockHandler> = {}) {
  return mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${P}`]: { body: { pages: PAGES } },
    [`GET ${P}/backlinks`]: {
      body: {
        pages: [
          { page: IDEAS.id, title: IDEAS.title, place: "private", edited_at: IDEAS.edited_at },
        ],
      },
    },
    [`GET ${P}/${HANDBOOK.id}`]: { body: page },
    [`PATCH ${P}/${HANDBOOK.id}`]: {
      body: { ...HANDBOOK, title: "Handbook v2", meta_revision: 2 },
    },
    [`POST ${P}/${HANDBOOK.id}/trash`]: {
      body: { ...HANDBOOK, trashed_at: "2026-10-13T09:00:00Z" },
    },
    ...extra,
  });
}

describe("a page open beside the tree", () => {
  it("shows its title, where it is, the editor and where it is linked from", async () => {
    api(view());
    renderOpen(HANDBOOK.id);
    const title = await screen.findByRole("textbox", { name: "Page title" });
    expect(title).toHaveValue(HANDBOOK.title);
    expect(screen.getByText("editor (editable)")).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Where this page is" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Holiday calendar" })).toBeTruthy();
    expect(await screen.findByText("Linked from 1 place")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Page icon: Document" })).toBeTruthy();
    expect(screen.getAllByRole("status").some((status) => status.textContent === "Saved")).toBe(
      true,
    );
  });

  it("saves a new title once typing pauses", async () => {
    const mocked = api(view());
    renderOpen(HANDBOOK.id);
    const title = await screen.findByRole("textbox", { name: "Page title" });
    fireEvent.change(title, { target: { value: "Handbook v2" } });
    await waitFor(
      () => {
        expect(
          mocked.calls.filter((call) => call.route === `PATCH ${P}/${HANDBOOK.id}`),
        ).toHaveLength(1);
      },
      { timeout: 3000 },
    );
    expect(
      mocked.calls.find((call) => call.route === `PATCH ${P}/${HANDBOOK.id}`)?.body,
    ).toMatchObject({
      title: "Handbook v2",
    });
  });

  it("changes the icon from the picker", async () => {
    const mocked = api(view());
    renderOpen(HANDBOOK.id);
    fireEvent.click(await screen.findByRole("button", { name: "Page icon: Document" }));
    fireEvent.click(screen.getByRole("button", { name: "Mail" }));
    await waitFor(
      () => {
        expect(mocked.calls.some((call) => call.route === `PATCH ${P}/${HANDBOOK.id}`)).toBe(true);
      },
      { timeout: 3000 },
    );
  });

  it("is read-only for someone who can only read it", async () => {
    api(view({ role: "reader" }));
    renderOpen(HANDBOOK.id);
    const title = await screen.findByRole("textbox", { name: "Page title" });
    expect(title).toHaveAttribute("readonly");
    expect(screen.getByText("editor (read-only)")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Page icon/u })).toBeNull();
  });

  it("says a page in the trash is, and cannot be edited", async () => {
    api(view({ trashed_at: "2026-10-13T09:00:00Z" }));
    renderOpen(HANDBOOK.id);
    expect(await screen.findByText("This page is in the trash.")).toBeTruthy();
    expect(screen.getByText("editor (read-only)")).toBeTruthy();
  });

  it("moves a page to the trash from the more menu and goes back to the tree", async () => {
    const mocked = api(view());
    const { router } = renderOpen(HANDBOOK.id);
    fireEvent.click(await screen.findByRole("button", { name: "More" }));
    fireEvent.click(screen.getByRole("button", { name: "Move to trash" }));
    await waitFor(() => {
      expect(mocked.calls.some((call) => call.route === `POST ${P}/${HANDBOOK.id}/trash`)).toBe(
        true,
      );
    });
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/pages`);
    });
  });

  it("says so when the page is not there", async () => {
    api(view(), {
      [`GET ${P}/${HANDBOOK.id}`]: { status: 404, body: { status: 404, title: "Not found" } },
    });
    renderOpen(HANDBOOK.id);
    expect(await screen.findByText("This page isn’t here")).toBeTruthy();
  });
});
