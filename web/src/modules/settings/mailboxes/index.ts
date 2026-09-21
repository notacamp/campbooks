/**
 * modules/settings/mailboxes — public barrel.
 *
 * Route integration:
 *   import { MailboxesPage } from "~/modules/settings/mailboxes";
 *
 *   // In your settings route tree (e.g. web/src/routes/settings.tsx):
 *   const mailboxesRoute = createRoute({
 *     getParentRoute: () => settingsLayoutRoute,
 *     path: "mailboxes",
 *     component: MailboxesPage,
 *     validateSearch: (search) => ({
 *       connected: search.connected as string | undefined,
 *       error: search.error as string | undefined,
 *     }),
 *   });
 *
 * Suggested sidebar nav entry:
 *   { label: "Mailboxes", path: "/settings/mailboxes", icon: Mail }
 *
 * Other modules must not import from here — mailbox data is private to the
 * settings surface.
 */
export { MailboxesPage } from "./MailboxesPage";
export type { ConnectedMailbox, MailboxProvider } from "./types";
