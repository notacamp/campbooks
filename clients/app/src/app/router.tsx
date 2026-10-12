import type { QueryClient } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";

import { buildModuleRoutes } from "./modules";
import { rootRoute } from "./routes/root";

const routeTree = rootRoute.addChildren(buildModuleRoutes(rootRoute));

/** The app's one router. */
export function createAppRouter(queryClient: QueryClient) {
  return createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: "intent",
    scrollRestoration: true,
  });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
