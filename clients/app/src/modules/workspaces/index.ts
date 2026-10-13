// Public surface of the `workspaces` module (mirrors crates/modules/workspaces).
// Other modules and the app shell import from here only.
export { useMembers, useWorkspaces, type Member, type Workspace } from "./api/queries";
export { buildWorkspacesRoutes } from "./routes";
export { WORKSPACES_SHORTCUTS } from "./shortcuts";
