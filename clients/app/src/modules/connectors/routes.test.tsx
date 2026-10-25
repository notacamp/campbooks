import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CONNECTION } from "../../../test/demo/identity";
import { ANA, DUARTE, SESSION } from "../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../test/render";

import { buildConnectorsRoutes, type MailboxConsent } from "./index";

const PAGE = `/w/${DUARTE.id}/settings/mailboxes`;

function renderAt(path: string) {
  const useConsent = vi.fn((): MailboxConsent => ({
    providers: ["google"],
    providersFailed: false,
    start: vi.fn(),
    starting: false,
    refusal: undefined,
    signInAgain: vi.fn(),
    wayInFate: () => "unchanged",
  }));
  mockApi({
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET /bff/api/v1/w/${DUARTE.id}/connections`]: { body: { connections: [CONNECTION] } },
    [`GET /bff/api/v1/w/${DUARTE.id}/members`]: { body: { members: [ANA] } },
  });
  const root = createRootRoute({ component: Outlet });
  const workspace = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: Outlet,
  });
  const history = createMemoryHistory({ initialEntries: [path] });
  const router = createRouter({
    routeTree: root.addChildren([
      workspace.addChildren(buildConnectorsRoutes(workspace, { useConsent })),
    ]),
    history,
  });
  renderWithProviders(<RouterProvider router={router} />);
  return { useConsent, history };
}

describe("buildConnectorsRoutes", () => {
  it("mounts Settings → Mailboxes with the front door, coming back to it", async () => {
    const { useConsent } = renderAt(PAGE);

    expect(await screen.findByRole("heading", { level: 1, name: "Mailboxes" })).toBeVisible();
    expect(useConsent).toHaveBeenCalledWith({ workspaceId: DUARTE.id, returnTo: PAGE });
  });

  it("opens Add a mailbox when asked to, then clears the address bar", async () => {
    const { history } = renderAt(`${PAGE}?add=1`);

    expect(await screen.findByRole("dialog", { name: "Add a mailbox" })).toBeVisible();
    await waitFor(() => {
      expect(history.location.search).toBe("");
    });
  });

  it("reads the front door's answer once, then clears it", async () => {
    const { history } = renderAt(`${PAGE}?connected=${CONNECTION.id}`);

    expect(await screen.findByText("Gmail ana@example.com is connected")).toBeVisible();
    await waitFor(() => {
      expect(history.location.search).toBe("");
    });
  });
});
