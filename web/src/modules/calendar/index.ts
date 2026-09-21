/**
 * modules/calendar — public barrel for the Calendar module.
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
  TimeAgenda,
  CalendarViewProps,
  AgendaItem,
  AgendaItemAction,
  DayNote,
  FocusSuggestion,
} from "./components";

// Data hook
export { useCalendarQuery, calendarKeys } from "./api";
