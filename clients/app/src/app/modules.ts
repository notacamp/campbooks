// The module manifest: every domain module plugs its routes and its
// shortcuts in here, and nowhere else. Modules are imported only through
// their public `index.ts`.
import type { AnyRoute } from "@tanstack/react-router";

import type { ShortcutDefinition } from "~/lib/shortcuts";
import { buildConnectorsRoutes } from "~/modules/connectors";
import { buildConversationsRoutes } from "~/modules/conversations";
import { buildSystemAdminRoutes, buildSystemRoutes, SYSTEM_ADMIN_SECTION } from "~/modules/system";
import { buildWorkspacesRoutes, WORKSPACES_SHORTCUTS } from "~/modules/workspaces";

export interface ModuleRouteParents<TRoot extends AnyRoute, TWorkspace extends AnyRoute> {
  /** The root route: pages outside any workspace (behind the session gate). */
  root: TRoot;
  /** `/w/$workspaceId`: pages inside a workspace's frame. */
  workspace: TWorkspace;
}

export function buildModuleRoutes<TRoot extends AnyRoute, TWorkspace extends AnyRoute>(
  parents: ModuleRouteParents<TRoot, TWorkspace>,
) {
  const { root, workspace } = parents;
  const workspaces = buildWorkspacesRoutes({ root, workspace });
  return {
    // Pages outside any workspace: status, first run, invitation links.
    root: [...buildSystemRoutes(root), ...workspaces.root],
    // A domain module's pages live inside a workspace (`/w/$workspaceId/...`).
    workspace: [
      ...workspaces.workspace,
      ...buildConversationsRoutes(workspace),
      ...buildConnectorsRoutes(workspace),
      // `cargo xtask new-module` inserts above this line.
    ],
  } as const;
}

/**
 * The admin section's pages (`/admin/...`, ADR 0015): each module mounts
 * its panels under the admin layout, mirroring the server, where each
 * module contributes its own admin operations.
 */
export function buildModuleAdminRoutes<TAdmin extends AnyRoute>(admin: TAdmin) {
  return [...buildSystemAdminRoutes(admin)] as const;
}

/** The admin sidebar, one entry per module panel (named `admin.sections.<id>`). */
export const ADMIN_SECTIONS = [SYSTEM_ADMIN_SECTION] as const;

/** Shortcuts modules define for their operations (keyed by operationId, ADR 0013). */
export const MODULE_SHORTCUTS: readonly ShortcutDefinition[] = [...WORKSPACES_SHORTCUTS];
