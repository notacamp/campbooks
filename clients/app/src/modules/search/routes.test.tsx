import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { buildConversationsRoutes } from "~/modules/conversations";

import {
  INBOX,
  LEDGER,
  LEDGER_DETAIL,
  ledgerBody,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  STREAM_COUNTS,
  W,
} from "../../../test/demo/conversations";
import { INVOICE_HIT, LEDGER_HIT, Q4_HIT, results } from "../../../test/demo/search";
import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../test/render";

import { forgetSearches, rememberSearch } from "./model";
import { buildSearchRoutes } from "./routes";

function renderAt(path: string) {
  const root = createRootRoute({ component: Outlet });
  const workspace = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: Outlet,
  });
  const router = createRouter({
    routeTree: root.addChildren([
      workspace.addChildren([
        ...buildSearchRoutes(workspace),
        ...buildConversationsRoutes(workspace),
      ]),
    ]),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

const SEARCH = `/w/${DUARTE.id}/search`;

/** The searches asked, by their parameters. */
function backend(
  answer: (params: URLSearchParams) => ReturnType<typeof results> | { status: number } = () =>
    results([Q4_HIT, LEDGER_HIT, INVOICE_HIT]),
  extra: Parameters<typeof mockApi>[0] = {},
) {
  const asked: URLSearchParams[] = [];
  const api = mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/search`]: (request) => {
      const params = new URL(request.url).searchParams;
      asked.push(params);
      const body = answer(params);
      return "status" in body ? problem(body.status, "query_too_long") : { body };
    },
    [`GET ${W}/conversations`]: () => ({ body: { conversations: PRIMARY, next_cursor: null } }),
    [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
    [`GET ${W}/conversations/${LEDGER.id}`]: { body: LEDGER_DETAIL },
    [`GET ${W}/conversations/${Q4.id}/messages/${Q4_DETAIL.messages[2]?.id ?? ""}/body`]: {
      body: Q4_BODY,
    },
    [`GET ${W}/conversations/${LEDGER.id}/messages/${LEDGER_DETAIL.messages[0]?.id ?? ""}/body`]: {
      body: ledgerBody(false),
    },
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`POST ${W}/conversations/${Q4.id}/archive`]: { body: { ...Q4, archived: true } },
    [`POST ${W}/conversations/${Q4.id}/unarchive`]: { body: Q4 },
    ...extra,
  });
  return { api, asked };
}

describe("the results page", () => {
  afterEach(() => {
    forgetSearches();
  });

  it("lists the hits newest first with the matched words marked, then by relevance on request", async () => {
    const { asked } = backend();
    const { router } = renderAt(`${SEARCH}?q=invoice`);

    const list = await screen.findByRole("region", { name: "Search results" });
    const row = await within(list).findByRole("button", { name: /Cloudhost Billing/u });
    expect(within(row).getAllByText("invoice")[0]?.tagName).toBe("MARK");
    expect(asked[0]?.get("sort")).toBe("newest");
    expect(asked[0]?.has("prefix")).toBe(false);
    expect(asked[0]?.get("limit")).toBe("20");

    fireEvent.click(screen.getByRole("button", { name: "Most relevant" }));

    await waitFor(() => {
      expect(asked.at(-1)?.get("sort")).toBe("relevance");
    });
    expect(router.state.location.search).toEqual({ q: "invoice", sort: "relevance" });
  });

  it("loads more with the server's cursor, unchanged", async () => {
    const cursor = "opaque/Cursor==.sig-1";
    const { asked } = backend((params) =>
      params.has("cursor") ? results([INVOICE_HIT]) : results([Q4_HIT], { next_cursor: cursor }),
    );
    renderAt(`${SEARCH}?q=invoice`);

    fireEvent.click(await screen.findByRole("button", { name: "More results" }));

    await waitFor(() => {
      expect(asked).toHaveLength(2);
    });
    expect(asked[1]?.get("cursor")).toBe(cursor);
    expect(asked[1]?.get("q")).toBe("invoice");
    expect(await screen.findByRole("button", { name: /Cloudhost Billing/u })).toBeVisible();
  });

  it("moves with J and K, archives with E and takes it back with Z", async () => {
    const { api } = backend();
    renderAt(`${SEARCH}?q=invoice`);
    const first = await screen.findByRole("button", { name: /Maya Okafor/u });
    // The conversations' own summaries arrive for the actions.
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === `GET ${W}/conversations`)).toBe(true);
    });

    fireEvent.keyDown(document.body, { key: "j" });
    await waitFor(() => {
      expect(first).toHaveAttribute("data-selected", "true");
    });
    fireEvent.keyDown(document.body, { key: "e" });

    await waitFor(() => {
      expect(
        api.calls.some((call) => call.route === `POST ${W}/conversations/${Q4.id}/archive`),
      ).toBe(true);
    });
    fireEvent.keyDown(document.body, { key: "z" });
    await waitFor(() => {
      expect(
        api.calls.some((call) => call.route === `POST ${W}/conversations/${Q4.id}/unarchive`),
      ).toBe(true);
    });
  });

  it("opens a hit with the reader beside the list, and Back returns to the results", async () => {
    backend();
    const { router } = renderAt(`${SEARCH}?q=invoice`);

    fireEvent.click(await screen.findByRole("button", { name: /Maya Okafor/u }));

    const reader = await screen.findByRole("main", { name: "Conversation" });
    expect(await within(reader).findByRole("heading", { name: Q4.subject })).toBeVisible();
    expect(router.state.location.pathname).toBe(`${SEARCH}/${Q4.id}`);
    expect(router.state.location.search).toEqual({ q: "invoice" });
    expect(screen.getByRole("region", { name: "Search results" })).toBeInTheDocument();

    fireEvent.click(within(reader).getByRole("button", { name: /Back to results/u }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(SEARCH);
    });
    expect(router.state.location.search).toEqual({ q: "invoice" });
  });

  it("adds and removes the quick filters in the query", async () => {
    const { asked } = backend();
    renderAt(`${SEARCH}?q=invoice`);
    await screen.findByRole("button", { name: /Maya Okafor/u });

    fireEvent.click(screen.getByRole("button", { name: "Unread" }));

    expect(screen.getByRole("searchbox", { name: "Search conversations" })).toHaveValue(
      "invoice is:unread",
    );
    await waitFor(() => {
      expect(asked.at(-1)?.get("q")).toBe("invoice is:unread");
    });
  });

  it("shows the recent searches and the tips for an empty query", async () => {
    rememberSearch("lease");
    const { asked } = backend();
    renderAt(SEARCH);

    expect(await screen.findByRole("heading", { name: "Recent searches" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Narrow it down" })).toBeVisible();
    expect(asked).toHaveLength(0);
    expect(await screen.findByRole("searchbox")).toHaveFocus();
  });

  it("says when nothing matches and a chip could go", async () => {
    backend(() =>
      results([], {
        query: { text: "zzz", filters: [{ operator: "from", value: "ana" }], warnings: [] },
      }),
    );
    renderAt(`${SEARCH}?q=zzz%20from:ana`);

    expect(
      await screen.findByRole("heading", { name: "No conversations match “zzz from:ana”" }),
    ).toBeVisible();
    expect(screen.getByText(/removing a filter above/u)).toBeVisible();
    expect(screen.getByRole("button", { name: "Remove filter: From ana" })).toBeVisible();
  });

  it("says that older mail is still being indexed", async () => {
    backend(() => results([Q4_HIT], { indexing: true }));
    renderAt(`${SEARCH}?q=invoice`);

    expect(await screen.findByText("Still indexing older mail")).toBeVisible();
  });

  it("says what was not understood", async () => {
    backend(() =>
      results([Q4_HIT], {
        query: {
          text: "invoice",
          filters: [],
          warnings: [{ code: "unknown_operator", operator: "frm" }],
        },
      }),
    );
    renderAt(`${SEARCH}?q=invoice%20frm:ana`);

    expect(await screen.findByText(/“frm:” is not a search operator/u)).toBeVisible();
  });

  it("explains a search that is too long", async () => {
    backend(() => ({ status: 400 }));
    renderAt(`${SEARCH}?q=invoice`);

    expect(await screen.findByRole("heading", { name: "That search is too long" })).toBeVisible();
  });
});

describe("a stale cursor", () => {
  it("offers to search again from the top", async () => {
    const asked: URLSearchParams[] = [];
    mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
      [`GET ${W}/conversations`]: { body: { conversations: PRIMARY, next_cursor: null } },
      [`GET ${W}/search`]: (request) => {
        const params = new URL(request.url).searchParams;
        asked.push(params);
        return params.has("cursor")
          ? problem(400, "cursor_mismatch")
          : { body: results([Q4_HIT], { next_cursor: "c1" }) };
      },
    });
    renderAt(`${SEARCH}?q=invoice`);

    fireEvent.click(await screen.findByRole("button", { name: "More results" }));

    expect(
      await screen.findByRole("heading", { name: "These results are out of date" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Search again" }));
    await waitFor(() => {
      expect(asked.at(-1)?.has("cursor")).toBe(false);
    });
    expect(asked.length).toBeGreaterThan(2);
  });
});
