// Public surface of the `identity` module (mirrors crates/modules/identity).
// Other modules and the app shell import from here only.
export { useHeldGrants } from "./api/queries";
export { MailboxOffer } from "./components/MailboxOffer/MailboxOffer";
export { MailboxStep } from "./components/MailboxStep/MailboxStep";
export { buildIdentityRoutes } from "./routes";
export { IDENTITY_SHORTCUTS } from "./shortcuts";
