import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { NOT_FOUND_PROBLEM, OVERVIEW, REAUTH_PROBLEM } from "../../../test/demo/admin";
import { DUARTE, SESSION } from "../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../test/render";
import { createAppRouter } from "../router";

const OVERVIEW_ROUTE = "GET /bff/admin/v1/system/overview";

function renderApp(path: string) {
  const view = renderWithProviders(<p>placeholder</p>);
  const router = createAppRouter(view.queryClient, createMemoryHistory({ initialEntries: [path] }));
  view.rerender(<RouterProvider router={router} />);
  return { ...view, router };
}

function signedIn(overview: { status?: number; body: unknown }) {
  return mockApi({
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [OVERVIEW_ROUTE]: overview,
  });
}

async function openAccountMenu() {
  await screen.findByRole("heading", { name: "Nothing needs you yet" });
  fireEvent.click(screen.getByRole("button", { name: /Ana Duarte/u }));
  return screen.findByRole("dialog", { name: "Account" });
}

describe("the admin section", () => {
  it("is not offered to members, and asks the admin API only when the menu opens", async () => {
    const api = signedIn({ status: 404, body: NOT_FOUND_PROBLEM });
    renderApp(`/w/${DUARTE.id}`);
    await screen.findByRole("heading", { name: "Nothing needs you yet" });
    expect(api.calls.some((call) => call.route === OVERVIEW_ROUTE)).toBe(false);

    const menu = await openAccountMenu();
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === OVERVIEW_ROUTE)).toBe(true);
    });
    expect(within(menu).getByRole("button", { name: "Sign out" })).toBeVisible();
    expect(within(menu).queryByRole("link", { name: "Administration" })).toBeNull();
  });

  it("shows members the not-found page at /admin", async () => {
    signedIn({ status: 404, body: NOT_FOUND_PROBLEM });
    renderApp("/admin");

    expect(await screen.findByRole("heading", { name: "Page not found" })).toBeVisible();
  });

  it("takes an administrator from the account menu to System", async () => {
    signedIn({ body: OVERVIEW });
    const { router } = renderApp(`/w/${DUARTE.id}`);

    const menu = await openAccountMenu();
    const entry = await within(menu).findByRole("link", { name: "Administration" });
    expect(entry).toHaveAttribute("href", "/admin");
    fireEvent.click(entry);

    expect(await screen.findByRole("heading", { level: 1, name: "System" })).toBeVisible();
    expect(router.state.location.pathname).toBe("/admin/system");
    // The sidebar and the phone's tab bar both name it as the current page.
    for (const link of screen.getAllByRole("link", { name: "System" })) {
      expect(link).toHaveAttribute("aria-current", "page");
    }
    expect(await screen.findByText("Campbooks’ own")).toBeVisible();
  });

  it("asks for a fresh sign-in when the last one is too old, coming back here", async () => {
    signedIn({ status: 401, body: REAUTH_PROBLEM });
    renderApp("/admin/system");

    expect(await screen.findByRole("heading", { name: "Sign in again to continue" })).toBeVisible();
    expect(screen.getByRole("link", { name: "Sign in again" })).toHaveAttribute(
      "href",
      "/bff/login?return_to=%2Fadmin%2Fsystem&max_age=0",
    );
  });

  it("says when the server can't be reached", async () => {
    signedIn({ status: 503, body: { status: 503, code: "database_unavailable" } });
    renderApp("/admin");

    expect(await screen.findByRole("heading", { name: "Can’t open administration" })).toBeVisible();
  });
});
