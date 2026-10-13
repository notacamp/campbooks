import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { ConnectorsHome } from "./components/ConnectorsHome/ConnectorsHome";

export function buildConnectorsRoutes<TParent extends AnyRoute>(parent: TParent) {
  const home = createRoute({
    getParentRoute: () => parent,
    path: "/connectors",
    component: ConnectorsHome,
  });
  return [home] as const;
}
