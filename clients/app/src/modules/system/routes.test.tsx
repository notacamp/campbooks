import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders, stubApi } from "../../../test/render";

import { buildSystemRoutes } from "./index";

describe("buildSystemRoutes", () => {
  it("mounts the system status page at /system", async () => {
    stubApi({
      "/bff/api/v1/system/version": { body: { version: "0.1.0", commit: null } },
      "/bff/api/v1/system/health": { body: { status: "ok", database: "ok" } },
    });
    const root = createRootRoute({ component: Outlet });
    const router = createRouter({
      routeTree: root.addChildren(buildSystemRoutes(root)),
      history: createMemoryHistory({ initialEntries: ["/system"] }),
    });

    renderWithProviders(<RouterProvider router={router} />);

    expect(await screen.findByRole("heading", { name: "System status" })).toBeVisible();
  });
});
