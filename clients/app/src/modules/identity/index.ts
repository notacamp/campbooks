// Public surface of the `identity` module (mirrors crates/modules/identity).
// Other modules and the app shell import from here only.
export { useHeldGrants } from "./api/queries";
export { AccountAdminPanel } from "./components/AccountAdminPanel/AccountAdminPanel";
export { useMailboxConsent } from "./consent";
export { MailboxOffer } from "./components/MailboxOffer/MailboxOffer";
export { MailboxStep } from "./components/MailboxStep/MailboxStep";
export { SuspendedAccounts } from "./components/SuspendedAccounts/SuspendedAccounts";
export { buildIdentityRoutes } from "./routes";
export { IDENTITY_SHORTCUTS } from "./shortcuts";
