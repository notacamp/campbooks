import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Writing: `c` opens a new message (a client-only command, so its id is
 * outside the module namespace). Reply, Reply all and Forward belong to the
 * reader; ⌘↵, ⌘B, ⌘I and Esc to the composer (`lib/shortcuts`).
 */
export const COMPOSE_SHORTCUTS = [
  {
    id: "compose.new",
    keys: "c",
    scope: "global",
    group: "compose",
    label: "conversations:compose.shortcuts.newMessage",
  },
] as const satisfies readonly ShortcutDefinition[];
