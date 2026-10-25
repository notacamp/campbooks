import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Keys of the pages screens. `pages.create` is a new global sequence beside
 * `n t`: it makes a private page and opens it, from anywhere.
 */
export const PAGES_SHORTCUTS = [
  {
    id: "pages.create",
    keys: "n p",
    scope: "global",
    group: "general",
    label: "pages:shortcuts.create",
  },
] as const satisfies readonly ShortcutDefinition[];
