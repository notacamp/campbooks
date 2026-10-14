import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { AssistantHome } from "./components/AssistantHome/AssistantHome";

/** The module's pages; in the app, `parent` is the workspace layout (`/w/$workspaceId`). */
export function buildAssistantRoutes<TParent extends AnyRoute>(parent: TParent) {
  const home = createRoute({
    getParentRoute: () => parent,
    path: "/assistant",
    component: AssistantHome,
  });
  return [home] as const;
}
