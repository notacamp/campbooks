import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { mockApi, renderWithProviders } from "../../../test/render";

import { buildAssistantRoutes } from "./index";

const WS = "ws-1";
const W = `/bff/api/v1/w/${WS}`;
const PROFILE = { proactivity: "ask_first", asked_at: null, revision: 0 };

type Slot = Parameters<typeof buildAssistantRoutes>[0]["rules"];

function renderAt(
  path: string,
  locale: "en" | "pt-PT" = "en",
  slots: { rules?: Slot; people?: Slot } = {},
) {
  const root = createRootRoute({ component: Outlet });
  const workspace = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: Outlet,
  });
  const routes = buildAssistantRoutes({ root, workspace, ...slots });
  const router = createRouter({
    routeTree: root.addChildren([...routes.root, workspace.addChildren([...routes.workspace])]),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  renderWithProviders(<RouterProvider router={router} />, locale);
}

describe("buildAssistantRoutes", () => {
  it("mounts Scout's memory inside the workspace", async () => {
    mockApi({ [`GET ${W}/assistant/profile`]: { body: PROFILE } });
    renderAt(`/w/${WS}/settings/memory`);

    expect(await screen.findByRole("heading", { name: "Scout’s memory" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Answer the questions" })).toBeVisible();
  });

  it("lists the people you told Scout about after your rules", async () => {
    mockApi({ [`GET ${W}/assistant/profile`]: { body: PROFILE } });
    renderAt(`/w/${WS}/settings/memory`, "en", {
      rules: () => <p>The rules list</p>,
      people: () => <p>The notes list</p>,
    });

    const people = await screen.findByRole("heading", { name: "People you told Scout about" });
    const rules = screen.getByRole("heading", { name: "Your rules" });
    expect(rules.compareDocumentPosition(people) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("The notes list")).toBeVisible();
  });

  it("mounts the questions outside it, in the user's language", async () => {
    mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [] } },
      [`GET ${W}/senders`]: { body: { senders: [] } },
      [`GET ${W}/assistant/profile`]: { body: PROFILE },
    });
    renderAt(`/scout/${WS}`, "pt-PT");

    expect(
      await screen.findByRole("heading", { name: "De quem quer ver sempre o correio?" }),
    ).toBeVisible();
  });
});
