import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Keys of the assistant's screens (client-only commands, so their ids are
 * outside the module's namespace). The questions are one-question pages:
 * Enter goes on (wherever the focus is, but a button or a field, which
 * handle Enter themselves), `S` skips one, Escape is "Not now".
 */
export const ASSISTANT_SHORTCUTS = [
  {
    id: "scout.next",
    keys: "enter",
    scope: "focus",
    group: "general",
    label: "assistant:shortcuts.next",
  },
  {
    id: "scout.skip",
    keys: "s",
    scope: "focus",
    group: "general",
    label: "assistant:shortcuts.skip",
  },
] as const satisfies readonly ShortcutDefinition[];
