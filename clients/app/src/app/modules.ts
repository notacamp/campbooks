// The module manifest: every domain module plugs its routes and its
// shortcuts in here, and nowhere else. Modules are imported only through
// their public `index.ts`.
import type { AnyRoute } from "@tanstack/react-router";

import type { ShortcutDefinition } from "~/lib/shortcuts";
import { buildConnectorsRoutes } from "~/modules/connectors";
import { buildConversationsRoutes } from "~/modules/conversations";
import { buildIdentityRoutes, IDENTITY_SHORTCUTS, MailboxStep } from "~/modules/identity";
import { buildSystemRoutes } from "~/modules/system";
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
  return {
    // Signed in or not: signing in, creating an account.
    public: [...buildIdentityRoutes(parents.public)],
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

/** Shortcuts modules define for their operations (keyed by operationId, ADR 0013). */
export const MODULE_SHORTCUTS: readonly ShortcutDefinition[] = [
  ...WORKSPACES_SHORTCUTS,
  ...IDENTITY_SHORTCUTS,
];
