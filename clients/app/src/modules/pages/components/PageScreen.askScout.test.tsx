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

import { ScoutChatProvider, type OpenScoutChat } from "~/lib/scout-chat";

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, PAGES, plainDoc, viewOfDoc } from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { ANA, DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders, type MockHandler } from "../../../../test/render";

import { PagesPage } from "./PagesPage";

// The editor needs a real browser; the toolbar around it is what is tested here.
vi.mock("../editor/PageEditor", () => ({ PageEditor: () => <p>editor</p> }));

const P = `${W}/pages`;

function Route() {
  const { pageId } = useParams({ strict: false });
  return <PagesPage workspaceId={DUARTE.id} pageId={pageId} />;
}

function renderOpen(scout?: OpenScoutChat) {
  const root = createRootRoute({ component: Outlet });
  const open = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages/$pageId",
    component: Route,
  });
  const router = createRouter({
    routeTree: root.addChildren([open]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/pages/${HANDBOOK.id}`] }),
  });
  const tree = <RouterProvider router={router} />;
  return renderWithProviders(
    scout === undefined ? tree : <ScoutChatProvider open={scout}>{tree}</ScoutChatProvider>,
  );
}

function api(extra: Record<string, MockHandler> = {}) {
  return mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${P}`]: { body: { pages: PAGES, tables: false } },
    [`GET ${P}/backlinks`]: { body: { pages: [] } },
    [`GET ${P}/${HANDBOOK.id}`]: {
      body: viewOfDoc({ ...HANDBOOK, created_by: ANA.user_id }, plainDoc("Welcome")),
    },
    ...extra,
  });
}

const OFFERED: Record<string, MockHandler> = {
  [`GET ${P}/${HANDBOOK.id}/sharing`]: {
    body: { everyone: true, notion: false, place: "workspace", readers: [] },
  },
};

const more = async () => {
  await screen.findByRole("textbox", { name: "Page title" });
  fireEvent.click(screen.getByRole("button", { name: "More" }));
};

describe("Ask Scout on a page", () => {
  it("opens Scout's chat about the page from the More menu", async () => {
    api(OFFERED);
    const scout = vi.fn();
    renderOpen(scout);

    await more();
    fireEvent.click(await screen.findByRole("button", { name: /^Ask Scout/u }));

    expect(scout).toHaveBeenCalledWith({ type: "pages.page", id: HANDBOOK.id }, expect.anything());
    // The toolbar stays one row: it is in the menu, not beside it.
    expect(screen.queryByRole("button", { name: /^Ask Scout/u })).not.toBeInTheDocument();
  });

  it("is not in the menu when Scout is not there", async () => {
    api(OFFERED);
    renderOpen();
    await more();
    expect(await screen.findByRole("button", { name: "Move to trash" })).toBeVisible();
    expect(screen.queryByRole("button", { name: /^Ask Scout/u })).not.toBeInTheDocument();
  });

  it("is not in the menu while the server does not offer mail and Scout in pages", async () => {
    api({ [`GET ${P}/${HANDBOOK.id}/sharing`]: { status: 404, body: { code: "not_found" } } });
    renderOpen(vi.fn());
    await more();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Move to trash" })).toBeVisible();
    });
    expect(screen.queryByRole("button", { name: /^Ask Scout/u })).not.toBeInTheDocument();
  });
});
