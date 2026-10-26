import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../test/render";

import { buildConnectNotionRoute } from "./routes";

describe("buildConnectNotionRoute", () => {
  it("mounts /connect/notion outside any workspace", async () => {
    mockApi({
      "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
      [`POST /bff/api/v1/w/${DUARTE.id}/connections/notion/authorizations`]: {
        status: 404,
        body: { status: 404, title: "Not found" },
      },
    });
    const root = createRootRoute({ component: Outlet });
    const routes = buildConnectNotionRoute(root);
    expect(routes).toHaveLength(1);
    const router = createRouter({
      routeTree: root.addChildren([...routes]),
      history: createMemoryHistory({ initialEntries: ["/connect/notion"] }),
    });
    renderWithProviders(<RouterProvider router={router} />);

    // Not a member of this workspace: the page says so and goes back to "/".
    expect(await screen.findByText("Only members can connect Notion")).toBeVisible();
    screen.getByRole("button", { name: "Back to Campbooks" }).click();
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/");
    });
  });
});
