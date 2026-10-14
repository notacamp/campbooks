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

import { LEDGER, W } from "../../../../../test/demo/conversations";
import { INVOICE_HIT, LEDGER_HIT, results } from "../../../../../test/demo/search";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";
import { forgetSearches, rememberSearch } from "../../model";

import { SearchOverlay } from "./SearchOverlay";

function renderOverlay(initialQuery = "") {
  const onClose = vi.fn();
  const root = createRootRoute({
    component: () => (
      <>
        <Outlet />
        <SearchOverlay workspaceId={DUARTE.id} open initialQuery={initialQuery} onClose={onClose} />
      </>
    ),
  });
  const page = (path: string) =>
    createRoute({
      getParentRoute: () => root,
      path,
      validateSearch: (search: Record<string, unknown>): { q?: string } =>
        typeof search["q"] === "string" ? { q: search["q"] } : {},
      component: () => null,
    });
  const router = createRouter({
    routeTree: root.addChildren([
      page("/w/$workspaceId/search"),
      page("/w/$workspaceId/inbox/$stream/$conversationId"),
    ]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/inbox/primary`] }),
  });
  return { router, onClose, ...renderWithProviders(<RouterProvider router={router} />) };
}

const field = () => screen.findByRole("combobox", { name: "Search conversations" });
const type = (input: HTMLElement, value: string) => {
  fireEvent.change(input, { target: { value } });
};

describe("the search overlay", () => {
  afterEach(() => {
    forgetSearches();
  });

  it("asks by relevance with the last word as a prefix, and marks what matched", async () => {
    const asked: URLSearchParams[] = [];
    mockApi({
      [`GET ${W}/search`]: (request) => {
        asked.push(new URL(request.url).searchParams);
        return { body: results([INVOICE_HIT, LEDGER_HIT]) };
      },
    });
    renderOverlay();

    type(await field(), "invoice");

    const option = await screen.findByRole("option", { name: /Cloudhost Billing/u });
    // The subject and the snippet both mark the word.
    const marks = within(option).getAllByText("invoice");
    expect(marks).toHaveLength(2);
    expect(marks.every((mark) => mark.tagName === "MARK")).toBe(true);
    expect(asked).toHaveLength(1);
    expect(asked[0]?.get("q")).toBe("invoice");
    expect(asked[0]?.get("sort")).toBe("relevance");
    expect(asked[0]?.get("prefix")).toBe("true");
    expect(screen.getByText("2 results")).toBeInTheDocument();
  });

  it("moves with the arrows and opens the selected conversation in the inbox reader", async () => {
    mockApi({ [`GET ${W}/search`]: { body: results([INVOICE_HIT, LEDGER_HIT]) } });
    const { router, onClose } = renderOverlay();
    const input = await field();
    type(input, "invoice");
    const options = await screen.findAllByRole("option");

    expect(options[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input).toHaveAttribute("aria-activedescendant", options[1]?.id);
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "k", ctrlKey: true });
    expect(options[1]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/inbox/newsletters/${LEDGER.id}`);
    });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("opens all results with Ctrl+Enter, and remembers the query", async () => {
    mockApi({ [`GET ${W}/search`]: { body: results([INVOICE_HIT]) } });
    const { router } = renderOverlay();
    const input = await field();
    type(input, " invoice ");
    await screen.findByRole("option");

    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/search`);
    });
    expect(router.state.location.search).toEqual({ q: "invoice" });
  });

  it("shows the filters it understood as chips and removes one from the text", async () => {
    mockApi({
      [`GET ${W}/search`]: {
        body: results([INVOICE_HIT], {
          query: {
            text: "invoice",
            filters: [{ operator: "from", value: "ana" }],
            warnings: [],
          },
        }),
      },
    });
    renderOverlay();
    const input = await field();
    type(input, "invoice from:ana");

    fireEvent.click(await screen.findByRole("button", { name: "Remove filter: From ana" }));

    expect(input).toHaveValue("invoice");
    // Typing and the arrow keys carry on where they were.
    expect(input).toHaveFocus();
  });

  it("starts from the query ⌘K handed over", async () => {
    mockApi({ [`GET ${W}/search`]: { body: results([INVOICE_HIT]) } });
    renderOverlay("lease");

    expect(await field()).toHaveValue("lease");
    await screen.findByRole("option");
  });

  it("offers the device's recent searches and the tips while the field is empty", async () => {
    rememberSearch("lease");
    mockApi({ [`GET ${W}/search`]: { body: results([INVOICE_HIT]) } });
    renderOverlay();
    const input = await field();

    expect(screen.getByRole("heading", { name: "Recent searches" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Narrow it down" })).toBeVisible();
    expect(screen.getByText("has:attachment")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "lease" }));
    expect(input).toHaveValue("lease");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    expect(screen.queryByRole("heading", { name: "Recent searches" })).not.toBeInTheDocument();
  });

  it("says when older mail is still being indexed, and when nothing matches", async () => {
    mockApi({
      [`GET ${W}/search`]: (request) =>
        new URL(request.url).searchParams.get("q") === "zzz"
          ? { body: results([], { indexing: true }, "zzz") }
          : { body: results([INVOICE_HIT]) },
    });
    renderOverlay();
    const input = await field();

    type(input, "zzz");

    expect(await screen.findByText("Still indexing older mail")).toBeVisible();
    expect(
      await screen.findByRole("heading", { name: "No conversations match “zzz”" }),
    ).toBeVisible();
  });

  it("says plainly when the search is too long", async () => {
    mockApi({ [`GET ${W}/search`]: problem(400, "query_too_long") });
    renderOverlay();

    type(await field(), "invoice");

    expect(await screen.findByText("That search is too long")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Try again" })).not.toBeInTheDocument();
  });

  it("closes with Escape", async () => {
    mockApi({});
    const { onClose } = renderOverlay();

    fireEvent.keyDown(await field(), { key: "Escape" });

    expect(onClose).toHaveBeenCalled();
  });

  it("hints its keys", async () => {
    mockApi({});
    renderOverlay();
    await field();

    const dialog = screen.getByRole("dialog", { name: "Search" });
    expect(within(dialog).getByText("Move")).toBeVisible();
    expect(within(dialog).getByRole("button", { name: /All results/u })).toBeDisabled();
    expect(within(dialog).getByText("Close")).toBeVisible();
  });
});
