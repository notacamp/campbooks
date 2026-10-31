// Public surface of the `pages` module (mirrors crates/modules/pages).
// Other modules and the app shell import from here only.
export { usePagesAvailable } from "./api/queries";
export { buildPagesRoutes } from "./routes";
export { PAGES_SHORTCUTS } from "./shortcuts";
// Mail and Scout in pages (campbooks-cloud#364).
export { useMailScoutAvailable } from "./api/mail-scout";
export { SaveToPageDialog } from "./save-thread/SaveToPageDialog";
export type { SaveThread } from "./save-thread/thread";
export { usePageSubject, usePageTrail } from "./scout-subjects";
