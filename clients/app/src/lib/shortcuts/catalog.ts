import type { ShortcutDefinition } from "./registry";

/**
 * The default shortcuts, declared once. Screens bind handlers to these ids
 * with `useShortcut`; hints, the `?` sheet and the ⌘K menu read the keys from
 * here. These are client-only commands; screens that call a server
 * operation add shortcuts keyed by its OpenAPI operationId (ADR 0013) through
 * `ShortcutRegistry.define`, and the conflict check covers them too.
 */
export const DEFAULT_SHORTCUTS = [
  // Go to (G, then a letter)
  {
    id: "nav.home",
    keys: "g h",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.home",
  },
  {
    id: "nav.inbox",
    keys: "g i",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.inbox",
  },
  {
    id: "nav.schedule",
    keys: "g s",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.schedule",
  },
  {
    id: "nav.tasks",
    keys: "g t",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.tasks",
  },
  {
    id: "nav.documents",
    keys: "g d",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.documents",
  },
  {
    id: "nav.money",
    keys: "g m",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.money",
  },

  // Lists and the selected item
  { id: "list.next", keys: "j", scope: "list", group: "list", label: "shortcuts:list.next" },
  {
    id: "list.previous",
    keys: "k",
    scope: "list",
    group: "list",
    label: "shortcuts:list.previous",
  },
  { id: "list.open", keys: "enter", scope: "list", group: "list", label: "shortcuts:list.open" },
  { id: "item.reply", keys: "r", scope: "item", group: "item", label: "shortcuts:item.reply" },
  { id: "item.done", keys: "e", scope: "item", group: "item", label: "shortcuts:item.done" },
  { id: "item.later", keys: "l", scope: "item", group: "item", label: "shortcuts:item.later" },
  { id: "item.hold", keys: "h", scope: "item", group: "item", label: "shortcuts:item.hold" },
  { id: "item.paid", keys: "p", scope: "item", group: "item", label: "shortcuts:item.paid" },
  { id: "item.choice1", keys: "1", scope: "item", group: "item", label: "shortcuts:item.choice1" },
  { id: "item.choice2", keys: "2", scope: "item", group: "item", label: "shortcuts:item.choice2" },

  // Writing
  {
    id: "compose.draft",
    keys: "mod+j",
    scope: "compose",
    group: "compose",
    label: "shortcuts:compose.draft",
    allowInInputs: true,
  },
  {
    id: "compose.send",
    keys: "mod+enter",
    scope: "compose",
    group: "compose",
    label: "shortcuts:compose.send",
    allowInInputs: true,
  },

  // Everywhere
  {
    id: "command.open",
    keys: "mod+k",
    scope: "global",
    group: "general",
    label: "shortcuts:command.open",
    allowInInputs: true,
  },
  { id: "undo", keys: "z", scope: "global", group: "general", label: "shortcuts:undo" },
  {
    id: "workspace.switch",
    keys: "mod+shift+w",
    scope: "global",
    group: "general",
    label: "shortcuts:workspace.switch",
  },
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

export type DefaultShortcutId = (typeof DEFAULT_SHORTCUTS)[number]["id"];

/** A shortcut id: one of the defaults, or one a module defined. */
export type ShortcutId = DefaultShortcutId | (string & {});
