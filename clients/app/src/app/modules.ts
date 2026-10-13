// The module manifest: every domain module plugs its routes and its
// shortcuts in here, and nowhere else. Modules are imported only through
// their public `index.ts`.
import type { AnyRoute } from "@tanstack/react-router";

import type { ShortcutDefinition } from "~/lib/shortcuts";
import { buildConnectorsRoutes, CONNECTORS_SHORTCUTS } from "~/modules/connectors";
import {
  buildConversationsRoutes,
  COMPOSE_SHORTCUTS,
  CONVERSATIONS_SHORTCUTS,
} from "~/modules/conversations";
import {
  buildIdentityRoutes,
  IDENTITY_SHORTCUTS,
  MailboxStep,
  useMailboxConsent,
} from "~/modules/identity";
import { buildSystemAdminRoutes, buildSystemRoutes, SYSTEM_ADMIN_SECTION } from "~/modules/system";
import { buildWorkspacesRoutes, WORKSPACES_SHORTCUTS } from "~/modules/workspaces";

export interface ModuleRouteParents<
  TPublic extends AnyRoute,
  TRoot extends AnyRoute,
  TWorkspace extends AnyRoute,
> {
  /** Pages anyone can open, signed in or not (the sign-in screens). */
  public: TPublic;
  /** Pages outside any workspace, behind the session gate. */
  root: TRoot;
  /** `/w/$workspaceId`: pages inside a workspace's frame. */
  workspace: TWorkspace;
}

export function buildModuleRoutes<
  TPublic extends AnyRoute,
  TRoot extends AnyRoute,
  TWorkspace extends AnyRoute,
>(parents: ModuleRouteParents<TPublic, TRoot, TWorkspace>) {
  const { root, workspace } = parents;
  // After a workspace is created, `identity` offers the mailbox held since sign-up.
  const workspaces = buildWorkspacesRoutes({ root, workspace, afterCreate: MailboxStep });
  const identity = buildIdentityRoutes({ public: parents.public, workspace });
  return {
    // Signed in or not: signing in, creating an account.
    public: [...identity.public],
    // Pages outside any workspace: status, first run, invitation links.
    root: [...buildSystemRoutes(root), ...workspaces.root],
    // A domain module's pages live inside a workspace (`/w/$workspaceId/...`).
    workspace: [
      ...workspaces.workspace,
      ...identity.workspace,
      ...buildConversationsRoutes(workspace),
      // `identity` is the front door for connecting a mailbox (as on the server).
      ...buildConnectorsRoutes(workspace, { useConsent: useMailboxConsent }),
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
export const MODULE_SHORTCUTS: readonly ShortcutDefinition[] = [
  ...WORKSPACES_SHORTCUTS,
  ...CONVERSATIONS_SHORTCUTS,
  ...COMPOSE_SHORTCUTS,
  ...IDENTITY_SHORTCUTS,
  ...CONNECTORS_SHORTCUTS,
];
