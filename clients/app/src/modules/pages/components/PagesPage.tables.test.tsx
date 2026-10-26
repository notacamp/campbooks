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
import { describe, expect, it, vi } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, PAGES, plainDoc, viewOfDoc } from "../../../../test/demo/pages";
import { CLIENTS_ID, P, ROW, TablesServer, VIEW_BOARD } from "../../../../test/demo/tables";
import { DUARTE } from "../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../test/render";
import { tablesApi } from "../../../../test/tables";

import { PagesPage } from "./PagesPage";

// The editor needs a real browser; the page around it is what is tested here.
vi.mock("../editor/PageEditor", () => ({
  PageEditor: ({
    editable,
    subPages,
    tables,
  }: {
    editable: boolean;
    subPages?: boolean;
    tables?: unknown;
  }) => (
    <p>
      editor {editable ? "editable" : "read-only"}; sub-pages {String(subPages)}; tables{" "}
      {tables === undefined ? "off" : "on"}
    </p>
  ),
}));

function Route() {
  const { pageId } = useParams({ strict: false });
  const search: { view?: string; row?: string } = useSearch({ strict: false });
  return <PagesPage workspaceId={DUARTE.id} pageId={pageId} view={search.view} row={search.row} />;
}

function renderAt(path: string) {
  const root = createRootRoute({ component: Outlet });
  const list = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages",
    component: Route,
  });
  const open = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages/$pageId",
    validateSearch: (search: Record<string, unknown>): { view?: string; row?: string } => ({
      ...(typeof search["view"] === "string" ? { view: search["view"] } : {}),
      ...(typeof search["row"] === "string" ? { row: search["row"] } : {}),
    }),
    component: Route,
  });
  const router = createRouter({
    routeTree: root.addChildren([list, open]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/${path}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

/** The tables server, with the demo workspace's ordinary pages in its tree. */
function api(server = new TablesServer(), tables = true) {
  return tablesApi(server, {
    [`GET ${W}/pages`]: () => ({ body: { pages: [...PAGES, ...server.pages()], tables } }),
    [`GET ${W}/pages/${HANDBOOK.id}`]: () => ({ body: viewOfDoc(HANDBOOK, plainDoc("Welcome")) }),
  });
}

describe("tables in the tree", () => {
  it("shows a table with its own icon, a New table button, and no sub-page button on it", async () => {
    api();
    renderAt("pages");
    expect(await screen.findByRole("button", { name: "Clients" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "New table" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add a sub-page to Clients" })).toBeNull();
    expect(
      screen.getByRole("button", { name: `Add a sub-page to ${HANDBOOK.title}` }),
    ).toBeTruthy();
  });

  it("makes a table with New table and opens it, with Undo", async () => {
    const { server } = api();
    const { router } = renderAt("pages");
    fireEvent.click(await screen.findByRole("button", { name: "New table" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toMatch(/\/pages\/0192f1a0-/u);
    });
    expect(server.pages().length).toBe(3);
    expect(await screen.findByText("Table created")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.calls.some((call) => call.endsWith("/trash"))).toBe(true);
    });
  });

  it("offers no tables while they are off", async () => {
    api(new TablesServer(), false);
    renderAt("pages");
    await screen.findByRole("button", { name: HANDBOOK.title });
    expect(screen.queryByRole("button", { name: "New table" })).toBeNull();
  });
});

describe("a table page", () => {
  it("opens as the table: the grid, not an editor", async () => {
    api();
    renderAt(`pages/${CLIENTS_ID}`);
    expect(await screen.findByRole("grid")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Page title" })).toHaveValue("Clients");
    expect(screen.queryByText(/^editor /u)).toBeNull();
  });
});

describe("the address of a table page", () => {
  it("holds the view and the row beside the table, so a link opens them", async () => {
    api();
    const { router } = renderAt(`pages/${CLIENTS_ID}?view=${VIEW_BOARD}&row=${ROW.lumen}`);
    expect(await screen.findByRole("tab", { name: "By stage", selected: true })).toBeTruthy();
    const peek = await screen.findByRole("dialog", { name: "Row" });
    expect(within(peek).getByRole("textbox", { name: "Row title" })).toHaveValue("Lumen Dental");
    fireEvent.click(within(peek).getByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(router.state.location.search).toEqual({ view: VIEW_BOARD });
    });
    fireEvent.click(screen.getByRole("tab", { name: "All clients" }));
    await waitFor(() => {
      expect(router.state.location.search).toEqual({ view: expect.any(String) as unknown });
    });
    fireEvent.click(await screen.findByRole("button", { name: "Open Harbour Physio" }));
    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({ row: ROW.harbour });
    });
  });
});

describe("a row opened as a page", () => {
  it("shows its properties under the title and its page below, with no sub-pages", async () => {
    const { server } = api();
    renderAt(`pages/${ROW.acme}`);
    const stage = await screen.findByRole("group", { name: "Stage" });
    expect(within(stage).getByText("Kickoff")).toBeTruthy();
    expect(screen.getByText(/editor editable; sub-pages false; tables off/u)).toBeTruthy();
    // A property edits in place.
    fireEvent.click(
      within(await screen.findByRole("group", { name: "Value" })).getByRole("button"),
    );
    const input = await screen.findByRole("textbox", { name: "Value — Acme Studio" });
    fireEvent.change(input, { target: { value: "10000" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("10000");
    });
  });
});

describe("an ordinary page", () => {
  it("offers sub-pages and the table blocks while tables are on, and not while they are off", async () => {
    api();
    const on = renderAt(`pages/${HANDBOOK.id}`);
    expect(await screen.findByText(/sub-pages true; tables on/u)).toBeTruthy();
    on.unmount();
    api(new TablesServer(), false);
    renderAt(`pages/${HANDBOOK.id}`);
    expect(await screen.findByText(/sub-pages true; tables off/u)).toBeTruthy();
  });
});
