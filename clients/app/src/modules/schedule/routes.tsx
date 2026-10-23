import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { ScheduleHome } from "./components/ScheduleHome/ScheduleHome";

/** The module's pages; in the app, `parent` is the workspace layout (`/w/$workspaceId`). */
export function buildScheduleRoutes<TParent extends AnyRoute>(parent: TParent) {
  const home = createRoute({
    getParentRoute: () => parent,
    path: "/schedule",
    component: ScheduleHome,
  });
  return [home] as const;
}
