// Public surface of the `connectors` module (mirrors crates/modules/connectors).
// Other modules and the app shell import from here only.
export { useConnections } from "./api/queries";
export {
  ConnectedMailboxes,
  type InAppLink,
} from "./components/ConnectedMailboxes/ConnectedMailboxes";
export type {
  ConsentRefusal,
  ConsentStart,
  MailboxConsent,
  UseMailboxConsent,
  WayInFate,
} from "./consent";
export { WorkspaceConnectionsAdmin } from "./components/WorkspaceConnectionsAdmin/WorkspaceConnectionsAdmin";
export { buildConnectNotionRoute, buildConnectorsRoutes } from "./routes";
export { useNotionSource } from "./notion-source";
export { CONNECTORS_SHORTCUTS } from "./shortcuts";
