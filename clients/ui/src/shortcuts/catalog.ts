import type { ShortcutDefinition } from "./registry";

/**
 * The shortcuts the design system itself speaks of: moving through a list,
 * the command menu, the `?` sheet, closing an overlay and undo (the undo
 * toast binds `undo`). An app defines its own next to these (or its own
 * versions of them) in one `ShortcutRegistry`; their labels are in the
 * package's `shortcuts` namespace, which the app merges its labels into.
 * Outside any `ShortcutProvider`, components fall back to a registry of
 * these.
 */
export const UI_SHORTCUTS = [
  {
    id: "list.next",
    keys: "j",
    alternateKeys: ["arrowdown"],
    scope: "list",
    group: "list",
    label: "shortcuts:list.next",
  },
  {
    id: "list.previous",
    keys: "k",
    alternateKeys: ["arrowup"],
    scope: "list",
    group: "list",
    label: "shortcuts:list.previous",
  },
  { id: "list.open", keys: "enter", scope: "list", group: "list", label: "shortcuts:list.open" },
  {
    id: "command.open",
    keys: "mod+k",
    scope: "global",
    group: "general",
    label: "shortcuts:command.open",
    allowInInputs: true,
  },
  { id: "undo", keys: "z", scope: "global", group: "general", label: "shortcuts:undo" },
  { id: "sheet.open", keys: "?", scope: "global", group: "general", label: "shortcuts:sheet.open" },
  {
    id: "overlay.close",
    keys: "escape",
    scope: "global",
    group: "general",
    label: "shortcuts:overlay.close",
    allowInInputs: true,
  },
] as const satisfies readonly ShortcutDefinition[];

export type UiShortcutId = (typeof UI_SHORTCUTS)[number]["id"];

/**
 * A shortcut id. The package knows no app's catalogue, so any string; an app
 * narrows it for its own code (Campbooks: its defaults, or a module's id).
 */
export type ShortcutId = string;
