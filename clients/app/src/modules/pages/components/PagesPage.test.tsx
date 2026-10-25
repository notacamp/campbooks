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
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import {
  BUDGET,
  HANDBOOK,
  HOLIDAYS,
  IDEAS,
  OLD_NOTES,
  ONBOARDING,
  PAGES,
} from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";

import { PagesPage } from "./PagesPage";

const P = `${W}/pages`;

function Route() {
  const { pageId } = useParams({ strict: false });
  const search: { new?: 1 } = useSearch({ strict: false });
  // The open page is the editor's business (it needs a real browser): the
  // tree beside it is what is tested here.
  return pageId === undefined ? (
    <PagesPage workspaceId={DUARTE.id} creating={search.new === 1} />
  ) : (
    <p>Opened {pageId}</p>
  );
}

function renderPages(path = "pages") {
  const root = createRootRoute({ component: Outlet });
  const list = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages",
    validateSearch: (search: Record<string, unknown>): { new?: 1 } =>
      search["new"] === 1 ? { new: 1 } : {},
    component: Route,
  });
  const open = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages/$pageId",
    component: Route,
  });
  const router = createRouter({
    routeTree: root.addChildren([list, open]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/${path}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

const MADE_ID = "0192f1a0-0000-7000-8000-0000000009ee";

function api(start = [...PAGES]) {
  let live = start;
  let trashed = [OLD_NOTES];
  const posts: unknown[] = [];
  const routes = mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${P}`]: (request) => ({
      body: {
        pages: new URL(request.url).searchParams.get("trashed") === "true" ? trashed : live,
      },
    }),
    [`POST ${P}`]: (_request, body) => {
      posts.push(body);
      const made = { ...IDEAS, id: MADE_ID, title: "" };
      live = [...live, made];
      return {
        status: 201,
        body: {
          ...made,
          created_at: made.edited_at,
          edited_by: made.created_by,
          revision: 1,
          ancestors: [],
          grants: [],
          document: { update: "AAA=", state_vector: "AA==" },
        },
      };
    },
    [`POST ${P}/${OLD_NOTES.id}/restore`]: () => {
      live = [...live, OLD_NOTES];
      trashed = [];
      return { body: OLD_NOTES };
    },
  });
  return { routes, posts };
}

describe("the pages tree", () => {
  it("shows Workspace, Private and Shared with you, with sub-pages folded", async () => {
    api();
    renderPages();
    expect(await screen.findByText("Team handbook")).toBeTruthy();
    expect(screen.getByText("Workspace")).toBeTruthy();
    expect(screen.getByText("Private")).toBeTruthy();
    expect(screen.getByText("Shared with you")).toBeTruthy();
    expect(screen.getByText(IDEAS.title)).toBeTruthy();
    expect(screen.getByText(BUDGET.title)).toBeTruthy();
    expect(screen.queryByText(ONBOARDING.title)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show the sub-pages of Team handbook" }));
    expect(await screen.findByText(ONBOARDING.title)).toBeTruthy();
    expect(screen.getByText(HOLIDAYS.title)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Hide the sub-pages of Team handbook" }));
    expect(screen.queryByText(ONBOARDING.title)).toBeNull();
  });

  it("offers no sub-page to a page the member can only read", async () => {
    api();
    renderPages();
    await screen.findByText(BUDGET.title);
    expect(screen.queryByRole("button", { name: `Add a sub-page to ${BUDGET.title}` })).toBeNull();
    expect(
      screen.getByRole("button", { name: `Add a sub-page to ${HANDBOOK.title}` }),
    ).toBeTruthy();
  });

  it("makes a page with New page and opens it", async () => {
    const { posts } = api();
    const { router } = renderPages();
    await screen.findByText("Team handbook");
    fireEvent.click(screen.getByRole("button", { name: /^New page/u }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/pages/${MADE_ID}`);
    });
    expect(posts).toEqual([{ place: "private" }]);
  });

  it("makes a page when opened with ?new=1 (the global sequence), once", async () => {
    const { posts } = api();
    const { router } = renderPages("pages?new=1");
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/pages/${MADE_ID}`);
    });
    expect(posts).toHaveLength(1);
  });

  it("adds a sub-page under a page", async () => {
    const { posts } = api();
    renderPages();
    await screen.findByText("Team handbook");
    fireEvent.click(screen.getByRole("button", { name: `Add a sub-page to ${HANDBOOK.title}` }));
    await waitFor(() => {
      expect(posts).toEqual([{ parent: HANDBOOK.id }]);
    });
  });

  it("says so when there are no pages yet", async () => {
    api([]);
    renderPages();
    expect(await screen.findByText("Write it down once")).toBeTruthy();
  });

  it("lists the trash and restores a page from it", async () => {
    api();
    renderPages();
    await screen.findByText("Team handbook");
    fireEvent.click(screen.getByRole("button", { name: "Trash" }));
    expect(await screen.findByText(OLD_NOTES.title)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Restore" }));
    await waitFor(() => {
      expect(screen.queryByText(OLD_NOTES.title)).toBeNull();
    });
  });

  it("asks before deleting for good", async () => {
    api();
    renderPages();
    await screen.findByText("Team handbook");
    fireEvent.click(screen.getByRole("button", { name: "Trash" }));
    await screen.findByText(OLD_NOTES.title);
    fireEvent.click(screen.getByRole("button", { name: "Delete for good" }));
    expect(await screen.findByText(/can’t be undone/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Keep it" }));
    expect(screen.queryByText(/can’t be undone/u)).toBeNull();
  });
});
