import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LEDGER, W } from "../../../test/demo/conversations";
import { INVOICE_HIT, LEDGER_HIT, results } from "../../../test/demo/search";
import { DUARTE, SESSION } from "../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../test/render";
import { forgetPrefs } from "~/lib/storage/prefs";
import { CommandMenu, type MenuQuery, type MenuSection } from "~/lib/ui";

import { recentKey, recentSearches, rememberSearch } from "./model";
import { useConversationSection } from "./use-conversation-section";

const SIGNED_IN = { "GET /bff/session": { body: SESSION } };

function renderMenu({
  path = `/w/${DUARTE.id}/inbox/primary`,
  useSection = useConversationSection,
}: {
  path?: string;
  useSection?: (menu: MenuQuery) => MenuSection | undefined;
} = {}) {
  const onClose = vi.fn();
  const onAsk = vi.fn();
  const root = createRootRoute({
    component: () => (
      <>
        <Outlet />
        <CommandMenu open onClose={onClose} onAsk={onAsk} useSection={useSection} />
      </>
    ),
  });
  const page = (route: string) =>
    createRoute({
      getParentRoute: () => root,
      path: route,
      validateSearch: (search: Record<string, unknown>): { q?: string } =>
        typeof search["q"] === "string" ? { q: search["q"] } : {},
      component: () => null,
    });
  const router = createRouter({
    routeTree: root.addChildren([
      page("/w/$workspaceId/search"),
      page("/w/$workspaceId/inbox/$stream"),
      page("/w/$workspaceId/inbox/$stream/$conversationId"),
      page("/welcome"),
    ]),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  return { router, onClose, onAsk, ...renderWithProviders(<RouterProvider router={router} />) };
}

const field = () => screen.findByRole("combobox", { name: "Search, ask, or run a command" });
const type = (input: HTMLElement, value: string) => {
  fireEvent.change(input, { target: { value } });
};

describe("the conversations in the command menu", () => {
  afterEach(() => {
    forgetPrefs("searches.");
  });

  it("asks by relevance with the last word as a prefix, lists hits first and marks what matched", async () => {
    const asked: URLSearchParams[] = [];
    mockApi({
      ...SIGNED_IN,
      [`GET ${W}/search`]: (request) => {
        asked.push(new URL(request.url).searchParams);
        return { body: results([INVOICE_HIT, LEDGER_HIT]) };
      },
    });
    renderMenu();

    type(await field(), "invoice");

    const option = await screen.findByRole("option", { name: /Cloudhost Billing/u });
    const marks = within(option).getAllByText("invoice");
    expect(marks).toHaveLength(2);
    expect(marks.every((mark) => mark.tagName === "MARK")).toBe(true);
    expect(asked).toHaveLength(1);
    expect(asked[0]?.get("q")).toBe("invoice");
    expect(asked[0]?.get("sort")).toBe("relevance");
    expect(asked[0]?.get("prefix")).toBe("true");
    const labels = screen.getAllByRole("option").map((each) => each.textContent);
    expect(labels[0]).toContain("Cloudhost Billing");
    expect(labels[2]).toContain("All results for “invoice”");
    expect(labels.at(-1)).toContain("Ask Scout: invoice");
    expect(screen.getByRole("group", { name: "Conversations" })).toBeInTheDocument();
  });

  it("shows all results when ↵ comes before the hits do", async () => {
    mockApi({ ...SIGNED_IN, [`GET ${W}/search`]: { body: results([INVOICE_HIT]) } });
    const { router, onClose, onAsk } = renderMenu();
    const input = await field();
    type(input, "invoice");
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/search`);
    });
    expect(router.state.location.search).toEqual({ q: "invoice" });
    expect(onClose).toHaveBeenCalled();
    expect(onAsk).not.toHaveBeenCalled();
  });

  it("moves across hits, commands and Ask with one cursor, and opens a hit in the inbox reader", async () => {
    mockApi({ ...SIGNED_IN, [`GET ${W}/search`]: { body: results([INVOICE_HIT, LEDGER_HIT]) } });
    const { router, onClose } = renderMenu();
    const input = await field();
    type(input, "invoice");
    await screen.findByRole("option", { name: /Cloudhost Billing/u });
    const options = screen.getAllByRole("option");

    expect(options[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input).toHaveAttribute("aria-activedescendant", options[1]?.id);
    fireEvent.keyDown(input, { key: "ArrowUp" });
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(input).toHaveAttribute("aria-activedescendant", options.at(-1)?.id);
    fireEvent.keyDown(input, { key: "j", ctrlKey: true });
    fireEvent.keyDown(input, { key: "j", ctrlKey: true });
    expect(options[1]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/inbox/newsletters/${LEDGER.id}`);
    });
    expect(onClose).toHaveBeenCalledOnce();
    expect(readRecent()).toEqual(["invoice"]);
  });

  it("opens all results with ⌘↵ or its row, and remembers the query", async () => {
    mockApi({ ...SIGNED_IN, [`GET ${W}/search`]: { body: results([INVOICE_HIT]) } });
    const { router } = renderMenu();
    const input = await field();
    type(input, " invoice ");
    await screen.findByRole("option", { name: /Cloudhost Billing/u });

    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/search`);
    });
    expect(router.state.location.search).toEqual({ q: "invoice" });
    expect(readRecent()).toEqual(["invoice"]);

    await router.navigate({ to: `/w/${DUARTE.id}/inbox/primary` });
    fireEvent.click(screen.getByRole("option", { name: "All results for “invoice”" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/search`);
    });
  });

  it("shows the filters it understood as chips and removes one from the text", async () => {
    mockApi({
      ...SIGNED_IN,
      [`GET ${W}/search`]: {
        body: results([INVOICE_HIT], {
          query: { text: "invoice", filters: [{ operator: "from", value: "ana" }], warnings: [] },
        }),
      },
    });
    renderMenu();
    const input = await field();
    type(input, "invoice from:ana");

    fireEvent.click(await screen.findByRole("button", { name: "Remove filter: From ana" }));

    expect(input).toHaveValue("invoice");
    expect(input).toHaveFocus();
  });

  it("offers the device's recent searches while the field is empty, and a pick fills it", async () => {
    rememberSearch(recentKey(SESSION.user.sub, DUARTE.id), "lease");
    mockApi({ ...SIGNED_IN, [`GET ${W}/search`]: { body: results([INVOICE_HIT]) } });
    renderMenu();
    const input = await field();

    // They appear once the session says whose they are.
    expect(await screen.findByRole("group", { name: "Recent searches" })).toBeVisible();
    fireEvent.click(screen.getByRole("option", { name: "lease" }));
    expect(input).toHaveValue("lease");
    expect(screen.getByRole("dialog", { name: "Command menu" })).toBeVisible();

    type(input, "");
    fireEvent.click(await screen.findByRole("option", { name: "Clear recent searches" }));
    await waitFor(() => {
      expect(screen.queryByRole("group", { name: "Recent searches" })).not.toBeInTheDocument();
    });
  });

  it("says when older mail is still being indexed, and when nothing matches", async () => {
    mockApi({
      ...SIGNED_IN,
      [`GET ${W}/search`]: (request) =>
        new URL(request.url).searchParams.get("q") === "zzz"
          ? { body: results([], { indexing: true }, "zzz") }
          : { body: results([INVOICE_HIT]) },
    });
    renderMenu();

    type(await field(), "zzz");

    expect(await screen.findByText("Still indexing older mail")).toBeVisible();
    expect(
      await screen.findByRole("heading", { name: "No conversations match “zzz”" }),
    ).toBeVisible();
    // The commands and Ask stay reachable.
    expect(screen.getByRole("option", { name: "Ask Scout: zzz" })).toBeVisible();
  });

  it("says plainly when the search is too long", async () => {
    mockApi({ ...SIGNED_IN, [`GET ${W}/search`]: problem(400, "query_too_long") });
    renderMenu();

    type(await field(), "invoice");

    expect(await screen.findByText("That search is too long")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Try again" })).not.toBeInTheDocument();
  });

  it("offers Try again for any other failure", async () => {
    mockApi({ ...SIGNED_IN, [`GET ${W}/search`]: problem(500, "internal") });
    renderMenu();

    type(await field(), "invoice");

    expect(await screen.findByRole("button", { name: "Try again" })).toBeVisible();
  });

  it("renders a hostile subject as text, never as markup", async () => {
    const hostile = {
      ...INVOICE_HIT,
      title: [{ text: "<img src=x onerror=alert(1)> <b>pay now</b>", hit: false }],
    };
    mockApi({ ...SIGNED_IN, [`GET ${W}/search`]: { body: results([hostile]) } });
    renderMenu();

    type(await field(), "pay");

    const option = await screen.findByRole("option", { name: /pay now/u });
    expect(option).toHaveTextContent("<img src=x onerror=alert(1)> <b>pay now</b>");
    expect(option.querySelector("img")).toBeNull();
    expect(option.querySelector("b")).toBeNull();
  });

  it("has no Conversations group outside a workspace", async () => {
    mockApi({ ...SIGNED_IN });
    renderMenu({ path: "/welcome" });

    type(await field(), "invoice");

    expect(screen.queryByRole("group", { name: "Conversations" })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Ask Scout: invoice" })).toBeVisible();
    expect(screen.queryByText("All results")).not.toBeInTheDocument();
  });

  it("hints its keys", async () => {
    mockApi({ ...SIGNED_IN });
    renderMenu();
    await field();
    type(screen.getByRole("combobox"), "invoice");

    const dialog = screen.getByRole("dialog", { name: "Command menu" });
    expect(within(dialog).getByText("Move")).toBeVisible();
    expect(within(dialog).getByText("Open")).toBeVisible();
    expect(within(dialog).getByText("All results")).toBeVisible();
    expect(within(dialog).getByText("Close")).toBeVisible();
  });
});

const readRecent = () => recentSearches(recentKey(SESSION.user.sub, DUARTE.id));
