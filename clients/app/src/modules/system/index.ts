// Public surface of the `system` module (mirrors crates/modules/system).
// Other modules and the app shell import from here only.
export { adminOverviewQuery } from "./api/admin";
export { buildSystemAdminRoutes, SYSTEM_ADMIN_SECTION } from "./admin";
export { buildSystemRoutes } from "./routes";
