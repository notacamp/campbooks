import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  ADMIN_ANA,
  ADMIN_ANA_ACCOUNT,
  ADMIN_ANA_DETAIL,
  ADMIN_CONNECTIONS,
  ADMIN_DUARTE,
  ADMIN_DUARTE_DETAIL,
  ADMIN_OLD_HOUSEHOLD,
  ADMIN_RUI,
  NOT_FOUND_PROBLEM,
  OVERVIEW,
} from "../../../test/demo/admin";
import { SESSION } from "../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../test/render";
import { createAppRouter } from "../router";

const ADMIN = "/bff/admin/v1";

function renderApp(path: string) {
  const view = renderWithProviders(<p>placeholder</p>);
  const router = createAppRouter(view.queryClient, createMemoryHistory({ initialEntries: [path] }));
  view.rerender(<RouterProvider router={router} />);
  return { ...view, router };
}

const PAGE_TWO = "page-two";

function administrator() {
  return mockApi({
    "GET /bff/session": { body: SESSION },
    [`GET ${ADMIN}/system/overview`]: { body: OVERVIEW },
    [`GET ${ADMIN}/workspaces/users`]: (request) => {
      const url = new URL(request.url);
      const q = url.searchParams.get("q");
      if (q !== null) {
        return { body: { users: q === "ru" ? [ADMIN_RUI] : [], next: null } };
      }
      return url.searchParams.get("after") === PAGE_TWO
        ? { body: { users: [ADMIN_ANA], next: null } }
        : { body: { users: [ADMIN_RUI], next: PAGE_TWO } };
    },
    [`GET ${ADMIN}/workspaces/users/${ADMIN_ANA.id}`]: { body: ADMIN_ANA_DETAIL },
    [`GET ${ADMIN}/identity/accounts/${ADMIN_ANA.account_id}`]: { body: ADMIN_ANA_ACCOUNT },
    [`GET ${ADMIN}/workspaces/users/${ADMIN_RUI.id}`]: { status: 404, body: NOT_FOUND_PROBLEM },
    [`GET ${ADMIN}/workspaces/workspaces`]: {
      body: { workspaces: [ADMIN_DUARTE, ADMIN_OLD_HOUSEHOLD], next: null },
    },
    [`GET ${ADMIN}/workspaces/workspaces/${ADMIN_DUARTE.id}`]: { body: ADMIN_DUARTE_DETAIL },
    [`GET ${ADMIN}/connectors/workspaces/${ADMIN_DUARTE.id}/connections`]: {
      body: { connections: ADMIN_CONNECTIONS },
    },
  });
}

describe("Admin → People", () => {
  it("lists people newest first, a page at a time, and finds them by name", async () => {
    const api = administrator();
    renderApp("/admin/people");

    expect(await screen.findByRole("heading", { level: 1, name: "People" })).toBeVisible();
    const list = await screen.findByRole("list", { name: "People" });
    expect(within(list).getByText("Rui Matos")).toBeVisible();
    expect(within(list).getByText(/rui@example\.com · Joined .* · 1 workspace$/u)).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Show more" }));
    expect(await within(list).findByText("Ana Duarte")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search people" }), {
      target: { value: "ru" },
    });
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === `GET ${ADMIN}/workspaces/users`)).toBe(true);
      const found = screen.getByRole("list", { name: "People" });
      expect(within(found).queryByText("Ana Duarte")).toBeNull();
      expect(within(found).getByText("Rui Matos")).toBeVisible();
    });

    fireEvent.change(screen.getByRole("searchbox", { name: "Search people" }), {
      target: { value: "zz" },
    });
    expect(
      await screen.findByRole("heading", { name: "No name or email address starts with “zz”." }),
    ).toBeVisible();
  });

  it("opens a person with their account and workspaces", async () => {
    administrator();
    const { router } = renderApp("/admin/people");

    fireEvent.click(await screen.findByRole("button", { name: "Show more" }));
    fireEvent.click(await screen.findByRole("link", { name: "Open Ana Duarte" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Ana Duarte" })).toBeVisible();
    expect(router.state.location.pathname).toBe(`/admin/people/${ADMIN_ANA.id}`);
    expect(await screen.findByText("Can sign in")).toBeVisible();
    const workspaces = screen.getByRole("list", { name: "Workspaces of Ana Duarte" });
    expect(within(workspaces).getByText("Duarte & Pessoa")).toBeVisible();
    expect(within(workspaces).getByText(/^Deleted /u)).toBeVisible();
    for (const link of screen.getAllByRole("link", { name: "People" })) {
      expect(link).toHaveAttribute("aria-current", "page");
    }
  });

  it("says when a person isn't there", async () => {
    administrator();
    renderApp(`/admin/people/${ADMIN_RUI.id}`);

    expect(await screen.findByRole("heading", { name: "This person isn’t here" })).toBeVisible();
    expect(screen.getByRole("link", { name: "All people" })).toHaveAttribute(
      "href",
      "/admin/people",
    );
  });
});

describe("Admin → Workspaces", () => {
  it("lists workspaces and opens one with its members and mailboxes", async () => {
    administrator();
    const { router } = renderApp("/admin/workspaces");

    const list = await screen.findByRole("list", { name: "Workspaces" });
    expect(
      within(list).getByText("Business · 2 members · Created", { exact: false }),
    ).toBeVisible();
    expect(within(list).getByText(/Personal · 1 member · Deleted/u)).toBeVisible();

    fireEvent.click(screen.getByRole("link", { name: "Open Duarte & Pessoa" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Duarte & Pessoa" })).toBeVisible();
    expect(router.state.location.pathname).toBe(`/admin/workspaces/${ADMIN_DUARTE.id}`);
    expect(screen.getByText("1 invitation")).toBeVisible();
    const members = screen.getByRole("list", { name: "Members of Duarte & Pessoa" });
    expect(within(members).getByText("Rui Matos")).toBeVisible();
    expect(
      await screen.findByText("Needs the member to reconnect", { exact: false }),
    ).toBeVisible();
  });
});
