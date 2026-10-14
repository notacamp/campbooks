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

import { buildAssistantRoutes } from "./index";

function renderAt(path: string, locale: "en" | "pt-PT" = "en") {
  const root = createRootRoute({ component: Outlet });
  const router = createRouter({
    routeTree: root.addChildren(buildAssistantRoutes(root)),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  renderWithProviders(<RouterProvider router={router} />, locale);
}

describe("buildAssistantRoutes", () => {
  it("mounts the assistant home page", async () => {
    renderAt("/assistant");

    expect(await screen.findByRole("heading", { name: "Assistant" })).toBeVisible();
  });

  it("renders in the user's language", async () => {
    renderAt("/assistant", "pt-PT");

    expect(await screen.findByText("Este módulo ainda não tem ecrãs.")).toBeVisible();
  });
});
