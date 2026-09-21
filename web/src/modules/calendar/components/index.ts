/**
 * modules/calendar/components — public surface.
 *
 * Exports the CalendarView component and its TypeScript types.
 * Consumers import from "~/modules/calendar/components".
 */

export {
  CalendarView,
  type CalendarViewProps,
  type TimeAgenda,
  type AgendaItem,
  type AgendaItemAction,
  type DayNote,
  type DayNoteFocus,
  type FocusSuggestion,
  type LateObligation,
  type PrepMeeting,
  type TaskExtra,
  type ReminderExtra,
  type EventExtra,
  type FocusBlockExtra,
} from "./calendar-view";
