import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Keys of the assistant's screens (client-only commands, so their ids are
 * outside the module's namespace). The questions are one-question pages:
 * `S` skips one, Enter goes on (the page's form), Escape is "Not now".
 */
export const ASSISTANT_SHORTCUTS = [
  {
    id: "scout.skip",
    keys: "s",
    scope: "focus",
    group: "general",
    label: "assistant:shortcuts.skip",
  },
] as const satisfies readonly ShortcutDefinition[];
