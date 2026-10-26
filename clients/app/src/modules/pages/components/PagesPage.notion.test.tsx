import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
  useParams,
  useSearch,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { NOTION_PAGES, PAGES } from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";
import type { UseNotionSource } from "../notion-source";

import { PagesPage } from "./PagesPage";

const P = `${W}/pages`;
const useSource: UseNotionSource = () => ({
  connections: [{ id: "c1", name: "Acme", needsReauth: false }],
  list: { status: "ready", pages: NOTION_PAGES, truncated: false, error: undefined, retry: () => undefined },
});

function Route({ withNotion }: { withNotion: boolean }) {
  const { pageId } = useParams({ strict: false });
  const search: { import?: "notion" } = useSearch({ strict: false });
  return pageId === undefined ? (
    <PagesPage
      workspaceId={DUARTE.id}
      importing={search.import === "notion"}
      notion={withNotion ? { useSource } : undefined}
    />
  ) : (
    <p>Opened {pageId}</p>
  );
}

function renderAt(path: string, withNotion = true) {
  mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${P}`]: { body: { pages: PAGES } },
    [`GET ${P}/notion/links`]: { body: { links: [] } },
    [`POST ${P}/notion/imports`]: {
      status: 201,
      body: { pages: [{ page: "new-1", notion_page: NOTION_PAGES[3]?.notion_page }], skipped: 0 },
    },
  });
  const root = createRootRoute({ component: Outlet });
  const list = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages",
    validateSearch: (search: Record<string, unknown>): { import?: "notion" } =>
      search["import"] === "notion" ? { import: "notion" } : {},
    component: () => <Route withNotion={withNotion} />,
  });
  const open = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages/$pageId",
    component: () => <Route withNotion={withNotion} />,
  });
  const router = createRouter({
    routeTree: root.addChildren([list, open]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/${path}`] }),
  });
  renderWithProviders(<RouterProvider router={router} />);
  return router;
}

describe("Pages and Notion", () => {
  it("opens the import from the Private group", async () => {
    renderAt("pages");
    fireEvent.click(await screen.findByRole("button", { name: "Import from Notion" }));
    expect(await screen.findByRole("dialog", { name: "Import from Notion" })).toBeVisible();
  });

  it("offers no import without Notion", async () => {
    renderAt("pages", false);
    await screen.findByText("Team handbook");
    expect(screen.queryByRole("button", { name: "Import from Notion" })).toBeNull();
  });

  it("opens the import when sent here from Settings, and leaves the address clean when it closes", async () => {
    const router = renderAt("pages?import=notion");
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(router.state.location.search).toEqual({});
    });
  });

  it("opens the first page it made", async () => {
    const router = renderAt("pages?import=notion");
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    fireEvent.click(within(dialog).getByLabelText("Meeting notes"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Import 1 page" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/pages/new-1`);
    });
  });
});
