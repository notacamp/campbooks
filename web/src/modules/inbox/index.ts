/**
 * modules/inbox — public barrel for the Inbox module.
 *
 * Exports the module config (for main.tsx registration) and the public surface
 * (types, hooks, mappers) that other modules and routes may consume.
 */

import type { ModuleConfig } from "~/modules";

// The inbox module contributes no extension points in this first cut.
// Routes are wired in router.tsx (buildInboxRoutes).
export const moduleConfig: ModuleConfig = {
  extensions: [],
};

// Public surface
export type { InboxRow, InboxTab } from "./types";
export { TAB_LABELS } from "./types";
export {
  useInboxInfiniteQuery,
  useArchiveMutation,
  inboxKeys,
  annotationFor,
  signalPlaceFor,
  useInboxRealtimeSync,
} from "./hooks";
