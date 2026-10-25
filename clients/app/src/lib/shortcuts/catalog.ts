import type { ShortcutDefinition } from "@notacamp/ui/shortcuts";

/**
 * The default shortcuts, declared once (the engine is @notacamp/ui's; this is
 * Campbooks' catalogue, which carries the design system's UI_SHORTCUTS as
 * they are, checked by registry.test.ts). Screens bind handlers to these ids
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
    id: "nav.pages",
    keys: "g p",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.pages",
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
  {
    id: "nav.settings",
    keys: "g ,",
    scope: "global",
    group: "navigation",
    label: "shortcuts:nav.settings",
  },

  // Lists and the selected item
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
  { id: "item.reply", keys: "r", scope: "item", group: "item", label: "shortcuts:item.reply" },
  { id: "item.done", keys: "e", scope: "item", group: "item", label: "shortcuts:item.done" },
  { id: "item.snooze", keys: "s", scope: "item", group: "item", label: "shortcuts:item.snooze" },
  { id: "item.hold", keys: "h", scope: "item", group: "item", label: "shortcuts:item.hold" },
  { id: "item.choice1", keys: "1", scope: "item", group: "item", label: "shortcuts:item.choice1" },
  { id: "item.choice2", keys: "2", scope: "item", group: "item", label: "shortcuts:item.choice2" },
  { id: "item.choice3", keys: "3", scope: "item", group: "item", label: "shortcuts:item.choice3" },
  // ←/→ walk the selected row's actions; Enter (list.open) presses the highlighted one.
  {
    id: "item.next_action",
    keys: "arrowright",
    scope: "item",
    group: "item",
    label: "shortcuts:item.nextAction",
  },
  {
    id: "item.previous_action",
    keys: "arrowleft",
    scope: "item",
    group: "item",
    label: "shortcuts:item.previousAction",
  },

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
  {
    id: "compose.bold",
    keys: "mod+b",
    scope: "compose",
    group: "compose",
    label: "shortcuts:compose.bold",
    allowInInputs: true,
  },
  {
    id: "compose.italic",
    keys: "mod+i",
    scope: "compose",
    group: "compose",
    label: "shortcuts:compose.italic",
    allowInInputs: true,
  },
  {
    // Asking in a chat (Scout's): Enter asks, Shift+Enter is a new line.
    id: "chat.send",
    keys: "enter",
    scope: "compose",
    group: "compose",
    label: "shortcuts:compose.ask",
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
