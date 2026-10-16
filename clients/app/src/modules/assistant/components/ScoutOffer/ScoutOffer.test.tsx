import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { ScoutOffer } from "./ScoutOffer";

const WS = "ws-1";
const W = `/bff/api/v1/w/${WS}`;
const PROFILE = { proactivity: "act_then_tell", asked_at: null, revision: 0 };

function renderHome() {
  const root = createRootRoute({ component: Outlet });
  const home = createRoute({
    getParentRoute: () => root,
    path: "/",
    component: () => <ScoutOffer workspaceId={WS} />,
  });
  const scout = createRoute({
    getParentRoute: () => root,
    path: "/scout/$workspaceId",
    component: () => <h1>The questions</h1>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, scout]),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  renderWithProviders(<RouterProvider router={router} />);
}

describe("ScoutOffer", () => {
  it("starts the questions with T", async () => {
    mockApi({ [`GET ${W}/assistant/profile`]: { body: PROFILE } });
    renderHome();

    expect(await screen.findByRole("button", { name: /Start/u })).toBeVisible();
    fireEvent.keyDown(document.body, { key: "t" });
    expect(await screen.findByRole("heading", { name: "The questions" })).toBeVisible();
  });

  it("puts them off with Shift+T, keeping the answer it had, and says so when that fails", async () => {
    const api = mockApi({
      [`GET ${W}/assistant/profile`]: { body: PROFILE },
      [`PUT ${W}/assistant/profile`]: problem(503, "unavailable"),
    });
    renderHome();

    await screen.findByRole("button", { name: /Not now/u });
    fireEvent.keyDown(document.body, { key: "T", shiftKey: true });
    expect(
      await screen.findByText("Scout couldn’t put the questions off. Try again."),
    ).toBeVisible();
    expect(api.calls.find((call) => call.route === `PUT ${W}/assistant/profile`)?.body).toEqual({
      proactivity: "act_then_tell",
      asked: true,
    });
    expect(screen.getByRole("button", { name: /Not now/u })).toBeVisible();
  });
});
