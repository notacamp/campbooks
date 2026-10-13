import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Shortcuts for `conversations` operations, keyed by their OpenAPI
 * operationId (ADR 0013), plus the client-only commands of the inbox
 * screens (ids outside the module namespace). Done (E), Reply (R), J/K,
 * Enter, Z and Escape are the app's defaults; the choices of a suggestion
 * use the defaults' 1 and 2.
 */
export const CONVERSATIONS_SHORTCUTS = [
  {
    id: "inbox.stream.primary",
    keys: "g 1",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.primary",
  },
  {
    id: "inbox.stream.updates",
    keys: "g 2",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.updates",
  },
  {
    id: "inbox.stream.receipts",
    keys: "g 3",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.receipts",
  },
  {
    id: "inbox.stream.newsletters",
    keys: "g 4",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.newsletters",
  },
  {
    id: "conversations.star",
    keys: "s",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.star",
  },
  {
    id: "conversations.mark_unread",
    keys: "u",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.unread",
  },
  {
    id: "conversations.move_to_stream",
    keys: "m",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.move",
  },
  {
    id: "item.reply_all",
    // A is Settings' "add a way in" (identity.start_link), live alongside.
    keys: "shift+r",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.replyAll",
  },
  {
    id: "item.forward",
    keys: "f",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.forward",
  },
  {
    id: "inbox.show_images",
    keys: "shift+i",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.showImages",
  },
] as const satisfies readonly ShortcutDefinition[];
