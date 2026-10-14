import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { SignupsAdminPanel } from "./components/SignupsAdminPanel/SignupsAdminPanel";

/**
 * The workspaces module's place in the admin section's navigation: who may
 * sign up (requests and codes). The shell names it (`admin.sections.signups`).
 */
export const SIGNUPS_ADMIN_SECTION = {
  id: "signups",
  to: "/admin/signups",
  icon: "key",
} as const;

/** The workspaces module's part of the admin section, under the admin layout. */
export function buildWorkspacesAdminRoutes<TParent extends AnyRoute>(parent: TParent) {
  const signups = createRoute({
    getParentRoute: () => parent,
    path: "/signups",
    component: SignupsAdminPanel,
  });
  return [signups] as const;
}
