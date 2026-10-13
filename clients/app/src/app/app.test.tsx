import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { readPref, writePref } from "~/lib/storage/prefs";

import { ANA, DUARTE, HOUSEHOLD, PENDING, SESSION, SOFIA } from "../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../test/render";

import { createAppRouter } from "./router";

const LIST = "GET /bff/api/v1/workspaces";

function renderApp(path: string) {
  const view = renderWithProviders(<p>placeholder</p>);
  const router = createAppRouter(view.queryClient, createMemoryHistory({ initialEntries: [path] }));
  view.rerender(<RouterProviderWithClient router={router} />);
  return { ...view, router };
}

// renderWithProviders wraps the providers; re-rendering swaps in the router.
function RouterProviderWithClient({ router }: { router: ReturnType<typeof createAppRouter> }) {
  return <RouterProvider router={router} />;
}

describe("the app", () => {
  afterEach(() => {
    // Back to a fresh device.
    writePref("workspace", "");
    writePref("theme", "system");
    writePref("density", "comfortable");
    writePref("locale", "en");
    delete document.documentElement.dataset["accent"];
  });

  it("shows the landing when signed out, signing in back to the same page", async () => {
    mockApi({ "GET /bff/session": { body: { authenticated: false } } });
    globalThis.history.replaceState(null, "", "/w/abc/settings/members");
    renderApp("/w/abc/settings/members");

    const signIn = await screen.findByRole("link", { name: "Sign in" });
    expect(signIn).toHaveAttribute("href", "/bff/login?return_to=%2Fw%2Fabc%2Fsettings%2Fmembers");
    globalThis.history.replaceState(null, "", "/");
  });

  it("uses invitation copy on an invitation link", async () => {
    mockApi({ "GET /bff/session": { body: { authenticated: false } } });
    globalThis.history.replaceState(null, "", "/invitations/accept?token=t0ken");
    renderApp("/invitations/accept?token=t0ken");

    expect(
      await screen.findByRole("heading", { name: "You’ve been invited to a workspace" }),
    ).toBeVisible();
    globalThis.history.replaceState(null, "", "/");
  });

  it("says when the server can't be reached", async () => {
    mockApi({ "GET /bff/session": { status: 503, body: { status: 503 } } });
    renderApp("/");

    expect(await screen.findByRole("heading", { name: "Can’t reach Campbooks" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  });

  it("starts a first run when there is no workspace", async () => {
    mockApi({ "GET /bff/session": { body: SESSION }, [LIST]: { body: { workspaces: [] } } });
    const { router } = renderApp("/");

    expect(
      await screen.findByRole("heading", { name: "What should we call your workspace?" }),
    ).toBeVisible();
    expect(router.state.location.pathname).toBe("/welcome");
  });

  it("opens the last workspace, with its accent and the empty Home", async () => {
    writePref("workspace", HOUSEHOLD.id);
    mockApi({
      "GET /bff/session": { body: SESSION },
      [LIST]: { body: { workspaces: [DUARTE, HOUSEHOLD] } },
    });
    const { router } = renderApp("/");

    expect(await screen.findByRole("heading", { name: "Nothing needs you yet" })).toBeVisible();
    expect(router.state.location.pathname).toBe(`/w/${HOUSEHOLD.id}`);
    await waitFor(() => {
      expect(document.documentElement.dataset["accent"]).toBe("personal");
    });
    expect(screen.getByRole("button", { name: /Connect your mail/u })).toBeDisabled();
    expect(screen.getByText("Coming soon")).toBeVisible();
  });

  it("switches workspace with Ctrl+Shift+W", async () => {
    mockApi({
      "GET /bff/session": { body: SESSION },
      [LIST]: { body: { workspaces: [DUARTE, HOUSEHOLD] } },
    });
    const { router } = renderApp(`/w/${DUARTE.id}`);
    await screen.findByRole("heading", { name: "Nothing needs you yet" });

    fireEvent.keyDown(document.body, { key: "W", ctrlKey: true, shiftKey: true });
    const menu = await screen.findByRole("dialog", { name: "Switch workspace" });
    fireEvent.click(within(menu).getByRole("button", { name: /Household/u }));

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${HOUSEHOLD.id}`);
    });
  });

  it("says when a workspace isn't available", async () => {
    mockApi({ "GET /bff/session": { body: SESSION }, [LIST]: { body: { workspaces: [DUARTE] } } });
    renderApp("/w/elsewhere");

    expect(
      await screen.findByRole("heading", { name: "This workspace isn’t available" }),
    ).toBeVisible();
  });

  it("opens Settings on Members and goes there with G then comma", async () => {
    mockApi({
      "GET /bff/session": { body: SESSION },
      [LIST]: { body: { workspaces: [DUARTE] } },
      [`GET /bff/api/v1/w/${DUARTE.id}/members`]: { body: { members: [ANA, SOFIA] } },
      [`GET /bff/api/v1/w/${DUARTE.id}/invitations`]: { body: { invitations: [PENDING] } },
    });
    const { router } = renderApp(`/w/${DUARTE.id}`);
    await screen.findByRole("heading", { name: "Nothing needs you yet" });

    fireEvent.keyDown(document.body, { key: "g" });
    fireEvent.keyDown(document.body, { key: "," });

    expect(await screen.findByRole("heading", { level: 1, name: "Members" })).toBeVisible();
    expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/settings/members`);
  });

  it("changes theme and language from the account menu, and signs out", async () => {
    const api = mockApi({
      "GET /bff/session": { body: SESSION },
      [LIST]: { body: { workspaces: [DUARTE] } },
      "POST /bff/logout": { body: { logout_url: null } },
    });
    renderApp(`/w/${DUARTE.id}`);
    await screen.findByRole("heading", { name: "Nothing needs you yet" });

    fireEvent.click(screen.getByRole("button", { name: /Ana Duarte/u }));
    const menu = await screen.findByRole("dialog", { name: "Account" });
    fireEvent.click(within(menu).getByRole("button", { name: "Dark" }));
    expect(document.documentElement.dataset["theme"]).toBe("dark");
    fireEvent.click(within(menu).getByRole("button", { name: "Compact" }));
    expect(document.documentElement.dataset["density"]).toBe("compact");
    fireEvent.change(within(menu).getByRole("combobox", { name: "Language" }), {
      target: { value: "fr" },
    });

    expect(readPref("locale")).toBe("fr");
    // (Tests bundle the English shell for every locale.)
    fireEvent.click(within(menu).getByRole("button", { name: "Sign out" }));
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === "POST /bff/logout")).toBe(true);
    });
  });
});
