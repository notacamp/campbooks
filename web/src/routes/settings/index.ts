/**
 * routes/settings — barrel + route builder.
 *
 * Integration: in router.tsx add ONE line to authRoute.addChildren([...]):
 *   ...buildSettingsRoutes(authRoute),
 *
 * Shape:
 *   /settings              → SettingsLayout (layout, redirects to /settings/account)
 *     /settings/account    → AccountPage    (fully implemented)
 *     /settings/workspace  → WorkspacePage  (fully implemented)
 *     /settings/privacy    → PrivacyPage    (fully implemented)
 *     /settings/members    → MembersPage    (stub)
 *     /settings/ai         → AiPage         (stub)
 *     /settings/memory     → MemoryPage     (stub)
 *     /settings/inbox      → InboxSettingsPage (stub)
 *     /settings/integrations → IntegrationsPage (stub)
 *     /settings/notifications → NotificationsPage (stub)
 *     /settings/plan       → PlanPage       (stub)
 *     /settings/mailboxes  → MailboxesPage  (stub — reserved for Mailboxes agent)
 *
 * The SETTINGS_NAV registry in SettingsNav.tsx is the single source of truth
 * for labels, icons, and paths. Routes here must match its `key` entries.
 *
 * Route rules (TanStack Router):
 *   1. Each sub-page's `getParentRoute` MUST return the settings LAYOUT route.
 *   2. Child paths are RELATIVE: "account" not "/account".
 *   3. The bare "/" index redirects to /settings/account so the pane is never empty.
 */
import { createRoute, redirect, type AnyRoute } from "@tanstack/react-router";
import { SettingsLayout } from "./SettingsLayout";
import { AccountPage } from "./AccountPage";
import { WorkspacePage } from "./WorkspacePage";
import { PrivacyPage } from "./PrivacyPage";
import { MembersPage } from "./MembersPage";
import { AiPage } from "./AiPage";
import { MemoryPage } from "./MemoryPage";
import { InboxSettingsPage } from "./InboxSettingsPage";
import { IntegrationsPage } from "./IntegrationsPage";
import { NotificationsPage } from "./NotificationsPage";
import { PlanPage } from "./PlanPage";
import { MailboxesPage } from "./MailboxesPage";

// ── Route builder ─────────────────────────────────────────────────────────────

export const buildSettingsRoutes = (parent: AnyRoute): AnyRoute[] => {
  // Layout route — wraps all settings sub-pages with the sub-nav shell.
  const layout = createRoute({
    getParentRoute: () => parent,
    path: "/settings",
    component: SettingsLayout,
  });

  // Index redirect: /settings → /settings/account
  const indexRoute = createRoute({
    getParentRoute: () => layout,
    path: "/",
    beforeLoad: () => {
      throw redirect({ to: "/settings/account" });
    },
  });

  // ── Fully implemented pages ──────────────────────────────────────────────
  const accountRoute = createRoute({
    getParentRoute: () => layout,
    path: "account",
    component: AccountPage,
  });

  const workspaceRoute = createRoute({
    getParentRoute: () => layout,
    path: "workspace",
    component: WorkspacePage,
  });

  const privacyRoute = createRoute({
    getParentRoute: () => layout,
    path: "privacy",
    component: PrivacyPage,
  });

  // ── Stub pages ───────────────────────────────────────────────────────────
  const membersRoute = createRoute({
    getParentRoute: () => layout,
    path: "members",
    component: MembersPage,
  });

  const aiRoute = createRoute({
    getParentRoute: () => layout,
    path: "ai",
    component: AiPage,
  });

  const memoryRoute = createRoute({
    getParentRoute: () => layout,
    path: "memory",
    component: MemoryPage,
  });

  const inboxRoute = createRoute({
    getParentRoute: () => layout,
    path: "inbox",
    component: InboxSettingsPage,
  });

  const integrationsRoute = createRoute({
    getParentRoute: () => layout,
    path: "integrations",
    component: IntegrationsPage,
  });

  const notificationsRoute = createRoute({
    getParentRoute: () => layout,
    path: "notifications",
    component: NotificationsPage,
  });

  const planRoute = createRoute({
    getParentRoute: () => layout,
    path: "plan",
    component: PlanPage,
  });

  const mailboxesRoute = createRoute({
    getParentRoute: () => layout,
    path: "mailboxes",
    component: MailboxesPage,
  });

  return [
    layout.addChildren([
      indexRoute,
      accountRoute,
      workspaceRoute,
      privacyRoute,
      membersRoute,
      aiRoute,
      memoryRoute,
      inboxRoute,
      integrationsRoute,
      notificationsRoute,
      planRoute,
      mailboxesRoute,
    ]),
  ];
};
