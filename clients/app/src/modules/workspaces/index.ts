// Public surface of the `workspaces` module (mirrors crates/modules/workspaces).
// Other modules and the app shell import from here only.
export {
  buildWorkspacesAdminRoutes,
  PEOPLE_ADMIN_SECTION,
  SIGNUPS_ADMIN_SECTION,
  WORKSPACES_ADMIN_SECTION,
} from "./admin";
export { useWorkspaces, type Workspace } from "./api/queries";
export { buildWorkspacesRoutes } from "./routes";
export { WORKSPACES_SHORTCUTS } from "./shortcuts";
