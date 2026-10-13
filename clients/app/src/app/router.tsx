import type { QueryClient } from "@tanstack/react-query";
import { createRoute, createRouter, Navigate, type RouterHistory } from "@tanstack/react-router";

import { buildModuleRoutes } from "./modules";
import { EmptyHome } from "./routes/home";
import { gatedRoute, rootRoute } from "./routes/root";
import { Start } from "./routes/start";
import { WorkspaceLayout } from "./routes/workspace";

const startRoute = createRoute({
  getParentRoute: () => gatedRoute,
  path: "/",
  component: Start,
});

const workspaceRoute = createRoute({
  getParentRoute: () => gatedRoute,
  path: "/w/$workspaceId",
  component: WorkspaceLayout,
});

const homeRoute = createRoute({
  getParentRoute: () => workspaceRoute,
  path: "/",
  component: EmptyHome,
});

/** Settings opens on its only section for now: Members. */
const settingsRoute = createRoute({
  getParentRoute: () => workspaceRoute,
  path: "/settings",
  component: function SettingsIndex() {
    const { workspaceId } = settingsRoute.useParams();
    return <Navigate to="/w/$workspaceId/settings/members" params={{ workspaceId }} replace />;
  },
});

const moduleRoutes = buildModuleRoutes({
  public: rootRoute,
  root: gatedRoute,
  workspace: workspaceRoute,
});

const routeTree = rootRoute.addChildren([
  gatedRoute.addChildren([
    startRoute,
    workspaceRoute.addChildren([homeRoute, settingsRoute, ...moduleRoutes.workspace]),
    ...moduleRoutes.root,
  ]),
  ...moduleRoutes.public,
]);

/** The app's one router (`history` defaults to the browser's; tests pass a memory one). */
export function createAppRouter(queryClient: QueryClient, history?: RouterHistory) {
  return createRouter({
    routeTree,
    ...(history === undefined ? {} : { history }),
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
