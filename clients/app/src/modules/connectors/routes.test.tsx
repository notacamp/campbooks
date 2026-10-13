import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "../../../test/render";

import { buildConnectorsRoutes } from "./index";

function renderAt(path: string, locale: "en" | "pt-PT" = "en") {
  const root = createRootRoute({ component: Outlet });
  const router = createRouter({
    routeTree: root.addChildren(buildConnectorsRoutes(root)),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  renderWithProviders(<RouterProvider router={router} />, locale);
}

describe("buildConnectorsRoutes", () => {
  it("mounts the connectors home page", async () => {
    renderAt("/connectors");

    expect(await screen.findByRole("heading", { name: "Connectors" })).toBeVisible();
  });

  it("renders in the user's language", async () => {
    renderAt("/connectors", "pt-PT");

    expect(await screen.findByText("Este módulo ainda não tem ecrãs.")).toBeVisible();
  });
});
