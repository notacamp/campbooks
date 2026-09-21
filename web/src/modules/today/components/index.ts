/**
 * modules/today/components — barrel.
 * Re-exports TodayView and its data types for consumers.
 *
 * Does NOT re-export from the parent modules/today/index.ts (that file is
 * owned by another session and should not be touched).
 */
export {
  TodayView,
  type TodayViewProps,
  type TodayData,
  type TodayAction,
  type NeedsYouItem,
  type ComingUpItem,
  type HandledCounts,
  type Greeting,
} from "./today-view";
