/**
 * modules/today — public barrel for the Today module.
 *
 * Exports the module config (for main.tsx registration) and the public
 * surface (types, hooks) that routes may consume.
 */

import type { ModuleConfig } from "~/modules";

export const moduleConfig: ModuleConfig = {
  extensions: [],
};

// Public types — re-exported from the design view (do not redefine).
export type {
  TodayData,
  TodayViewProps,
  TodayAction,
  NeedsYouItem,
  ComingUpItem,
  HandledCounts,
  Greeting,
} from "./components";

// Data hook
export { useTodayQuery, todayKeys } from "./api";
