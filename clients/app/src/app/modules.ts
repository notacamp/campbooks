// The module manifest: every domain module plugs its routes and its
// shortcuts in here, and nowhere else. Modules are imported only through
// their public `index.ts`.
import type { AnyRoute } from "@tanstack/react-router";

import type { ShortcutDefinition } from "~/lib/shortcuts";
import { ASSISTANT_SHORTCUTS, buildAssistantRoutes } from "~/modules/assistant";
import {
  buildConnectNotionRoute,
  buildConnectorsRoutes,
  CONNECTORS_SHORTCUTS,
  useNotionSource,
  WorkspaceConnectionsAdmin,
} from "~/modules/connectors";
import {
  buildConversationsRoutes,
  COMPOSE_SHORTCUTS,
  CONVERSATIONS_SHORTCUTS,
  PersonNotesList,
  RulesList,
  useAskQuote,
  useConversationSources,
} from "~/modules/conversations";
import {
  AccountAdminPanel,
  buildIdentityRoutes,
  IDENTITY_SHORTCUTS,
  MailboxStep,
  SuspendedAccounts,
  useMailboxConsent,
} from "~/modules/identity";
import { buildPagesRoutes, PAGES_SHORTCUTS } from "~/modules/pages";
import {
  buildScheduleRoutes,
  SCHEDULE_SHORTCUTS,
  useFreeSlots,
  useWritableCalendars,
} from "~/modules/schedule";
import { buildSearchRoutes, SEARCH_SHORTCUTS } from "~/modules/search";
import {
  buildSystemAdminRoutes,
  buildSystemRoutes,
  JOBS_ADMIN_SECTION,
  SYSTEM_ADMIN_SECTION,
} from "~/modules/system";
import { buildTasksRoutes, TASKS_SHORTCUTS, type HoldSources } from "~/modules/tasks";
import {
  buildWorkspacesAdminRoutes,
  buildWorkspacesRoutes,
  PEOPLE_ADMIN_SECTION,
  SIGNUPS_ADMIN_SECTION,
  WORKSPACES_ADMIN_SECTION,
  WORKSPACES_SHORTCUTS,
} from "~/modules/workspaces";

/**
 * Hold time (tasks) reads the member's free time and writable calendars:
 * the schedule module's, plugged in here so tasks never imports it.
 */
export const HOLD_SOURCES: HoldSources = { useFreeSlots, useWritableCalendars };

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
  // Scout's questions and memory, whose rules list and notes about people
  // are the conversations module's.
  const assistant = buildAssistantRoutes({
    root,
    workspace,
    rules: RulesList,
    people: PersonNotesList,
  });
  return {
    // Signed in or not: signing in, creating an account.
    public: [...identity.public],
    // Pages outside any workspace: status, first run, invitation links.
    // `/connect/notion` is where Notion's listing sends people (the connectors module's).
    root: [
      ...buildSystemRoutes(root),
      ...workspaces.root,
      ...assistant.root,
      ...buildConnectNotionRoute(root),
    ],
    // A domain module's pages live inside a workspace (`/w/$workspaceId/...`).
    workspace: [
      ...workspaces.workspace,
      ...identity.workspace,
      ...buildConversationsRoutes(workspace),
      ...buildSearchRoutes(workspace),
      // `identity` is the front door for connecting a mailbox (as on the server).
      ...buildConnectorsRoutes(workspace, { useConsent: useMailboxConsent }),
      ...assistant.workspace,
      // Tasks made from emails say where they came from: the conversations module knows;
      // Hold time reads the schedule module's free time and calendars.
      ...buildTasksRoutes(workspace, {
        useSources: useConversationSources,
        useQuote: useAskQuote,
        holdSources: HOLD_SOURCES,
      }),
      ...buildScheduleRoutes(workspace, {
        useConsent: useMailboxConsent,
        useSources: useConversationSources,
      }),
      // The import from Notion lists what the connectors module's connection can see.
      ...buildPagesRoutes(workspace, { useNotionSource }),
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
  return [
    ...buildSystemAdminRoutes(admin),
    // People and Workspaces show identity's account and connectors' health.
    ...buildWorkspacesAdminRoutes(admin, {
      AccountPanel: AccountAdminPanel,
      AccountStandings: SuspendedAccounts,
      ConnectionsPanel: WorkspaceConnectionsAdmin,
    }),
  ] as const;
}

/** The admin sidebar, one entry per module panel (named `admin.sections.<id>`). */
export const ADMIN_SECTIONS = [
  SYSTEM_ADMIN_SECTION,
  PEOPLE_ADMIN_SECTION,
  WORKSPACES_ADMIN_SECTION,
  SIGNUPS_ADMIN_SECTION,
  JOBS_ADMIN_SECTION,
] as const;

/** Shortcuts modules define for their operations (keyed by operationId, ADR 0013). */
export const MODULE_SHORTCUTS: readonly ShortcutDefinition[] = [
  ...WORKSPACES_SHORTCUTS,
  ...CONVERSATIONS_SHORTCUTS,
  ...COMPOSE_SHORTCUTS,
  ...SEARCH_SHORTCUTS,
  ...IDENTITY_SHORTCUTS,
  ...ASSISTANT_SHORTCUTS,
  ...CONNECTORS_SHORTCUTS,
  ...TASKS_SHORTCUTS,
  ...SCHEDULE_SHORTCUTS,
  ...PAGES_SHORTCUTS,
];
