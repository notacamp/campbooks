import { createRoute, useParams, type AnyRoute } from "@tanstack/react-router";

import { AdminPeoplePanel } from "./components/AdminPeople/AdminPeoplePanel";
import { AdminPersonPage, type AccountPanel } from "./components/AdminPeople/AdminPersonPage";
import {
  AdminWorkspacePage,
  type ConnectionsPanel,
} from "./components/AdminWorkspaces/AdminWorkspacePage";
import { AdminWorkspacesPanel } from "./components/AdminWorkspaces/AdminWorkspacesPanel";

/** The workspaces module's places in the admin section's navigation (`admin.sections.<id>`). */
export const PEOPLE_ADMIN_SECTION = { id: "people", to: "/admin/people", icon: "people" } as const;
export const WORKSPACES_ADMIN_SECTION = {
  id: "workspaces",
  to: "/admin/workspaces",
  icon: "home",
} as const;

/** What other modules contribute to these pages, handed in by the module manifest. */
export interface WorkspacesAdminParts {
  /** A person's account: how they sign in, and suspending it (identity). */
  AccountPanel: AccountPanel;
  /** A workspace's connected accounts and their health (connectors). */
  ConnectionsPanel: ConnectionsPanel;
}

/**
 * The workspaces module's part of the admin section: People and Workspaces,
 * each a list and a detail page, under the admin layout the shell provides.
 */
export function buildWorkspacesAdminRoutes<TParent extends AnyRoute>(
  parent: TParent,
  parts: WorkspacesAdminParts,
) {
  const people = createRoute({
    getParentRoute: () => parent,
    path: "/people",
    component: AdminPeoplePanel,
  });
  const person = createRoute({
    getParentRoute: () => parent,
    path: "/people/$userId",
    component: function PersonRoute() {
      const { userId } = useParams({ strict: false });
      return <AdminPersonPage userId={userId ?? ""} AccountPanel={parts.AccountPanel} />;
    },
  });
  const workspaces = createRoute({
    getParentRoute: () => parent,
    path: "/workspaces",
    component: AdminWorkspacesPanel,
  });
  const workspace = createRoute({
    getParentRoute: () => parent,
    path: "/workspaces/$workspaceId",
    component: function WorkspaceRoute() {
      const { workspaceId } = useParams({ strict: false });
      return (
        <AdminWorkspacePage
          workspaceId={workspaceId ?? ""}
          ConnectionsPanel={parts.ConnectionsPanel}
        />
      );
    },
  });
  return [people, person, workspaces, workspace] as const;
}
