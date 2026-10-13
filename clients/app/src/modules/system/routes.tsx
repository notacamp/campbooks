import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { SystemStatus } from "./components/SystemStatus/SystemStatus";

export function buildSystemRoutes<TParent extends AnyRoute>(parent: TParent) {
  const home = createRoute({
    getParentRoute: () => parent,
    path: "/system",
    component: SystemStatus,
  });
  return [home] as const;
}
