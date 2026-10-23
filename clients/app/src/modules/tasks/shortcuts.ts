import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Keys of the tasks screens. `tasks.create` is a new global sequence beside
 * `n w`: it opens Tasks with "Add a task…" focused, and in the reader it
 * makes a task from the email. The row keys reuse the shared item keys:
 * `item.done` (E, Paid for a bill), `item.snooze` (S, Later),
 * `conversations.resolve_need` (X, not needed) and the list's `undo` (Z).
 */
export const TASKS_SHORTCUTS = [
  {
    id: "tasks.create",
    keys: "n t",
    scope: "global",
    group: "general",
    label: "tasks:shortcuts.create",
  },
] as const satisfies readonly ShortcutDefinition[];
