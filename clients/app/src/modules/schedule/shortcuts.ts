import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Shortcuts of the Calendar place, keyed by their OpenAPI operationId
 * (ADR 0013) where they call one, else a client-only id. They live in the
 * `calendar` scope: the views take 1, 2 and 3 (the inbox uses the same digits
 * for the choices of a suggestion), and T, E and A mean other things on the
 * Inbox. New event is N then E because N W (a new workspace) is global and a
 * lone N would hide it. J, K, Enter, E (Done), H (Hold time) and S (Later)
 * are the app's defaults, bound by the task sheet.
 */
export const SCHEDULE_SHORTCUTS = [
  {
    id: "schedule.view.agenda",
    keys: "1",
    scope: "calendar",
    group: "general",
    label: "schedule:shortcuts.agenda",
  },
  {
    id: "schedule.view.week",
    keys: "2",
    scope: "calendar",
    group: "general",
    label: "schedule:shortcuts.week",
  },
  {
    id: "schedule.view.month",
    keys: "3",
    scope: "calendar",
    group: "general",
    label: "schedule:shortcuts.month",
  },
  {
    id: "schedule.today",
    keys: "t",
    scope: "calendar",
    group: "general",
    label: "schedule:shortcuts.today",
  },
  {
    id: "schedule.create_event",
    keys: "n e",
    scope: "calendar",
    group: "general",
    label: "schedule:shortcuts.create",
  },
  {
    id: "schedule.update_event",
    keys: "e",
    scope: "calendar",
    group: "item",
    label: "schedule:shortcuts.edit",
  },
  {
    id: "schedule.delete_event",
    keys: "backspace",
    scope: "calendar",
    group: "item",
    label: "schedule:shortcuts.remove",
  },
  {
    id: "schedule.push_event",
    keys: "a",
    scope: "calendar",
    group: "item",
    label: "schedule:shortcuts.push",
  },
  {
    id: "schedule.open_source",
    keys: "o",
    scope: "calendar",
    group: "item",
    label: "schedule:shortcuts.openEmail",
  },
] as const satisfies readonly ShortcutDefinition[];
