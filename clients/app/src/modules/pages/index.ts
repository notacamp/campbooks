// Public surface of the `pages` module (mirrors crates/modules/pages).
// Other modules and the app shell import from here only.
export { usePagesAvailable } from "./api/queries";
export { buildPagesRoutes } from "./routes";
export { PAGES_SHORTCUTS } from "./shortcuts";
export type { NotionSource, UseNotionSource } from "./notion-source";
