import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { SystemAdminPanel } from "./components/SystemAdminPanel/SystemAdminPanel";

/**
 * The system module's place in the admin section's navigation. The shell
 * names it (`admin.sections.system`) and lists it in the admin sidebar.
 */
export const SYSTEM_ADMIN_SECTION = {
  id: "system",
  to: "/admin/system",
  icon: "settings",
} as const;

/**
 * The system module's part of the admin section (`/admin/...`): its pages,
 * mounted under the admin layout the app shell provides.
 */
export function buildSystemAdminRoutes<TParent extends AnyRoute>(parent: TParent) {
  const system = createRoute({
    getParentRoute: () => parent,
    path: "/system",
    component: SystemAdminPanel,
  });
  return [system] as const;
}
