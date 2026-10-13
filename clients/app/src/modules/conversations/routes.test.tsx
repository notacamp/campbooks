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

import { buildConversationsRoutes } from "./index";

function renderAt(path: string, locale: "en" | "pt-PT" = "en") {
  const root = createRootRoute({ component: Outlet });
  const router = createRouter({
    routeTree: root.addChildren(buildConversationsRoutes(root)),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  renderWithProviders(<RouterProvider router={router} />, locale);
}

describe("buildConversationsRoutes", () => {
  it("mounts the conversations home page", async () => {
    renderAt("/conversations");

    expect(await screen.findByRole("heading", { name: "Conversations" })).toBeVisible();
  });

  it("renders in the user's language", async () => {
    renderAt("/conversations", "pt-PT");

    expect(await screen.findByText("Este módulo ainda não tem ecrãs.")).toBeVisible();
  });
});
