import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Settings → Mailboxes' keys. Connect and Reconnect go through `identity`'s
 * front door (client-only ids outside the module namespace); Disconnect is
 * `connectors.disconnect`. They live in the `settings` scope: `c` is a new
 * message everywhere and `r` replies in the reader, which this page never
 * shows. J and K choose a mailbox (the list's own keys).
 */
export const CONNECTORS_SHORTCUTS = [
  {
    id: "mailboxes.connect",
    keys: "shift+c",
    scope: "settings",
    group: "general",
    label: "connectors:shortcuts.connect",
  },
  {
    id: "mailboxes.reconnect",
    keys: "r",
    scope: "settings",
    group: "item",
    label: "connectors:shortcuts.reconnect",
  },
  {
    id: "connectors.disconnect",
    keys: "backspace",
    scope: "settings",
    group: "item",
    label: "connectors:shortcuts.disconnect",
  },
] as const satisfies readonly ShortcutDefinition[];
