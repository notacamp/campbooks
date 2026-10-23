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

import { buildScheduleRoutes } from "./index";

function renderAt(path: string, locale: "en" | "pt-PT" = "en") {
  const root = createRootRoute({ component: Outlet });
  const router = createRouter({
    routeTree: root.addChildren(buildScheduleRoutes(root)),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  renderWithProviders(<RouterProvider router={router} />, locale);
}

describe("buildScheduleRoutes", () => {
  it("mounts the schedule home page", async () => {
    renderAt("/schedule");

    expect(await screen.findByRole("heading", { name: "Calendar" })).toBeVisible();
  });

  it("renders in the user's language", async () => {
    renderAt("/schedule", "pt-PT");

    expect(await screen.findByText("Este módulo ainda não tem ecrãs.")).toBeVisible();
  });
});
