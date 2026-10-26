import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DUARTE, HOUSEHOLD } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { ConnectNotion } from "./ConnectNotion";

const START = (id: string) => `POST /bff/api/v1/w/${id}/connections/notion/authorizations`;
const ANSWER = {
  status: 201,
  body: { authorize_url: "/auth/start/notion?ticket=abc", expires_at: "2026-10-25T09:10:00Z" },
};

function renderPage() {
  const leave = vi.fn();
  const onBack = vi.fn();
  const root = createRootRoute({ component: Outlet });
  const page = createRoute({
    getParentRoute: () => root,
    path: "/connect/notion",
    component: () => <ConnectNotion leave={leave} onBack={onBack} />,
  });
  const welcome = createRoute({
    getParentRoute: () => root,
    path: "/welcome",
    component: () => <p>Welcome page</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([page, welcome]),
    history: createMemoryHistory({ initialEntries: ["/connect/notion"] }),
  });
  renderWithProviders(<RouterProvider router={router} />);
  return { leave, onBack };
}

describe("ConnectNotion", () => {
  it("starts the connect flow at once for a person with one workspace", async () => {
    const mocked = mockApi({
      "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
      [START(DUARTE.id)]: ANSWER,
    });
    const { leave } = renderPage();

    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/notion?ticket=abc");
    });
    expect(screen.getByText("Opening Notion…")).toBeVisible();
    expect(mocked.calls.find((c) => c.route.startsWith("POST"))?.body).toEqual({
      return_to: `/w/${DUARTE.id}/settings/mailboxes`,
    });
    // Asked once, though the page rendered more than once.
    expect(mocked.calls.filter((c) => c.route.startsWith("POST"))).toHaveLength(1);
  });

  it("asks which workspace when there are several, then starts it", async () => {
    const mocked = mockApi({
      "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE, HOUSEHOLD] } },
      [START(HOUSEHOLD.id)]: ANSWER,
    });
    const { leave } = renderPage();

    expect(await screen.findByText("Which workspace should Notion connect to?")).toBeVisible();
    expect(mocked.calls.some((c) => c.route.startsWith("POST"))).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: new RegExp(HOUSEHOLD.name, "u") }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/notion?ticket=abc");
    });
    expect(mocked.calls.find((c) => c.route.startsWith("POST"))?.body).toEqual({
      return_to: `/w/${HOUSEHOLD.id}/settings/mailboxes`,
    });
  });

  it("sends someone with no workspace to make one", async () => {
    mockApi({ "GET /bff/api/v1/workspaces": { body: { workspaces: [] } } });
    renderPage();
    expect(await screen.findByText("Welcome page")).toBeVisible();
  });

  it("says plainly that an instance without Notion has nothing to connect", async () => {
    mockApi({
      "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
      [START(DUARTE.id)]: problem(409, "provider_not_configured"),
    });
    const { onBack, leave } = renderPage();

    expect(await screen.findByText("Notion isn’t available on this instance")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Back to Campbooks" }));
    expect(onBack).toHaveBeenCalled();
    expect(leave).not.toHaveBeenCalled();
  });

  it("says only members can connect, with no retry", async () => {
    mockApi({
      "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
      [START(DUARTE.id)]: problem(403, "connection_needs_user"),
    });
    renderPage();
    expect(await screen.findByText("Only members can connect Notion")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });

  it("offers another try after a failure", async () => {
    let tries = 0;
    mockApi({
      "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
      [START(DUARTE.id)]: () => {
        tries += 1;
        return tries === 1 ? problem(500, "internal") : ANSWER;
      },
    });
    const { leave } = renderPage();

    expect(await screen.findByText("Notion couldn’t be opened")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/notion?ticket=abc");
    });
    expect(tries).toBe(2);
  });

  it("offers to load the workspaces again when they did not load", async () => {
    let loads = 0;
    mockApi({
      "GET /bff/api/v1/workspaces": () => {
        loads += 1;
        return loads === 1 ? problem(500, "internal") : { body: { workspaces: [DUARTE] } };
      },
      [START(DUARTE.id)]: ANSWER,
    });
    const { leave } = renderPage();
    expect(await screen.findByText("Your workspaces didn’t load")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalled();
    });
  });
});
