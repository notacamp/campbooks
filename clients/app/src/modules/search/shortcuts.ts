import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * The search shortcut. `/` opens the overlay (or, on a phone, the results
 * page) from anywhere in a workspace; typed in a field it is text. The id is
 * the one Decision 4 of the `search` change names.
 */
export const SEARCH_SHORTCUTS = [
  {
    id: "search.open",
    keys: "/",
    scope: "global",
    group: "general",
    label: "search:shortcuts.open",
  },
] as const satisfies readonly ShortcutDefinition[];
