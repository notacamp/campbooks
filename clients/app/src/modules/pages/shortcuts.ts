import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Keys of the pages screens. `pages.create` is a new global sequence beside
 * `n t`: it makes a private page and opens it, from anywhere. The `table`
 * keys work on a table page: `n r` makes a row and `⌘↵` opens the active one;
 * the grid itself takes the arrows, Enter and Delete (listed so `?` shows them).
 */
export const PAGES_SHORTCUTS = [
  {
    id: "pages.create",
    keys: "n g",
    scope: "global",
    group: "general",
    label: "pages:shortcuts.create",
  },
  {
    id: "pages.table.new_row",
    keys: "n r",
    scope: "table",
    group: "list",
    label: "pages:shortcuts.tableNewRow",
  },
  {
    id: "pages.table.open_row",
    keys: "mod+enter",
    scope: "table",
    group: "list",
    label: "pages:shortcuts.tableOpenRow",
  },
  {
    id: "pages.table.edit",
    keys: "enter",
    scope: "table",
    group: "list",
    label: "pages:shortcuts.tableEdit",
  },
  {
    id: "pages.table.move",
    keys: "arrowdown",
    alternateKeys: ["arrowup", "arrowleft", "arrowright"],
    scope: "table",
    group: "list",
    label: "pages:shortcuts.tableMove",
  },
  {
    id: "pages.table.clear",
    keys: "delete",
    alternateKeys: ["backspace"],
    scope: "table",
    group: "list",
    label: "pages:shortcuts.tableClear",
  },
] as const satisfies readonly ShortcutDefinition[];
