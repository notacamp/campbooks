import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { JobsAdminPanel } from "./components/JobsAdminPanel/JobsAdminPanel";
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

/** The jobs panel's place in the admin navigation (`admin.sections.jobs`). */
export const JOBS_ADMIN_SECTION = {
  id: "jobs",
  to: "/admin/jobs",
  icon: "clock",
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
  const jobs = createRoute({
    getParentRoute: () => parent,
    path: "/jobs",
    component: JobsAdminPanel,
  });
  return [system, jobs] as const;
}
