import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { readPref, writePref } from "~/lib/storage/prefs";

import { readTabValue, writeTabValue } from "~/lib/storage/tab";

import { CONNECTION, HELD_GRANT } from "../../test/demo/identity";
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

  it("sends a signed-out browser to the sign-in screen, which comes back to the same page", async () => {
    mockApi({ "GET /bff/session": { body: { authenticated: false } } });
    const { router } = renderApp("/w/abc/settings/members?tab=all");

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/sign-in");
    });
    expect(router.state.location.search).toEqual({ return_to: "/w/abc/settings/members?tab=all" });
    expect(await screen.findByRole("heading", { name: "Opening sign-in…" })).toBeVisible();
    expect(readTabValue("signIn.returnTo")).toBe("/w/abc/settings/members?tab=all");
    writeTabValue("signIn.returnTo", "");
  });

  it("says in place when a session ends while the app is open", async () => {
    let signedIn = true;
    mockApi({
      "GET /bff/session": () => ({ body: signedIn ? SESSION : { authenticated: false } }),
      [LIST]: { body: { workspaces: [DUARTE] } },
    });
    const { queryClient, router } = renderApp(`/w/${DUARTE.id}`);
    await screen.findByRole("heading", { level: 1, name: "Nothing needs you yet" });

    signedIn = false;
    await queryClient.refetchQueries({ queryKey: ["bff", "session"] });

    expect(await screen.findByRole("heading", { name: "You were signed out." })).toBeVisible();
    expect(screen.getByRole("link", { name: "Sign in again" })).toHaveAttribute(
      "href",
      `/sign-in?return_to=${encodeURIComponent(`/w/${DUARTE.id}`)}`,
    );
    expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}`);
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

  it("offers the mailbox held since sign-up on Home, then names it once connected", async () => {
    let held = [HELD_GRANT] as unknown[];
    let connections = [] as unknown[];
    mockApi({
      "GET /bff/session": { body: SESSION },
      [LIST]: { body: { workspaces: [DUARTE] } },
      "GET /bff/api/v1/identity/held-grants": () => ({ body: { items: held } }),
      [`GET /bff/api/v1/w/${DUARTE.id}/connections`]: () => ({ body: { connections } }),
      [`POST /bff/api/v1/w/${DUARTE.id}/identity/held-grants/${HELD_GRANT.id}/adopt`]: () => {
        held = [];
        connections = [CONNECTION];
        return { body: { connection: CONNECTION.id } };
      },
    });
    renderApp(`/w/${DUARTE.id}`);

    fireEvent.click(await screen.findByRole("button", { name: "Connect" }));

    const mail = (await screen.findByRole("heading", { name: "Your mail" })).closest("section");
    expect(mail).toHaveTextContent("ana@example.comSyncing");
    expect(screen.getByRole("link", { name: "Manage mailboxes" })).toHaveAttribute(
      "href",
      `/w/${DUARTE.id}/settings/mailboxes`,
    );
  });

  it("asks to connect the held mailbox right after the first workspace is created", async () => {
    let workspaces = [] as unknown[];
    mockApi({
      "GET /bff/session": { body: SESSION },
      [LIST]: () => ({ body: { workspaces } }),
      "POST /bff/api/v1/workspaces": () => {
        workspaces = [DUARTE];
        return { status: 201, body: DUARTE };
      },
      "GET /bff/api/v1/identity/held-grants": { body: { items: [HELD_GRANT] } },
      [`GET /bff/api/v1/w/${DUARTE.id}/connections`]: { body: { connections: [] } },
    });
    const { router } = renderApp("/welcome");

    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    expect(
      await screen.findByRole("heading", {
        name: "Your Gmail ana@example.com will be connected to this workspace",
      }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Not now/u }));

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}`);
    });
    expect(await screen.findByRole("heading", { name: "Connect ana@example.com" })).toBeVisible();
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
    fireEvent.click(screen.getByRole("link", { name: /Connect your mail/u }));
    expect(await screen.findByRole("heading", { level: 1, name: "Mailboxes" })).toBeVisible();
    expect(router.state.location.pathname).toBe(`/w/${HOUSEHOLD.id}/settings/mailboxes`);
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
