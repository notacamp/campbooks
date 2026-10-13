import type { QueryClient } from "@tanstack/react-query";
import { createRoute, createRouter, Navigate, type RouterHistory } from "@tanstack/react-router";

import { AdminLayout } from "./admin/AdminLayout";
import { buildModuleAdminRoutes, buildModuleRoutes } from "./modules";
import { EmptyHome } from "./routes/home";
import { rootRoute } from "./routes/root";
import { Start } from "./routes/start";
import { WorkspaceLayout } from "./routes/workspace";

const startRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: Start,
});

const workspaceRoute = createRoute({
  getParentRoute: () => rootRoute,
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

/** `/admin`: instance administration, outside any workspace (ADR 0015). */
const adminRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/admin",
  component: AdminLayout,
});

/** Administration opens on its first panel: System. */
const adminIndexRoute = createRoute({
  getParentRoute: () => adminRoute,
  path: "/",
  component: function AdminIndex() {
    return <Navigate to="/admin/system" replace />;
  },
});

const moduleRoutes = buildModuleRoutes({ root: rootRoute, workspace: workspaceRoute });

const routeTree = rootRoute.addChildren([
  startRoute,
  workspaceRoute.addChildren([homeRoute, settingsRoute, ...moduleRoutes.workspace]),
  adminRoute.addChildren([adminIndexRoute, ...buildModuleAdminRoutes(adminRoute)]),
  ...moduleRoutes.root,
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
