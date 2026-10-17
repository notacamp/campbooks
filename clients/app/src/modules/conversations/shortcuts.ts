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
    // Pin: the provider's star (Gmail STARRED), named Pin in the UI.
    id: "conversations.star",
    keys: "p",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.star",
  },
  {
    id: "conversations.resolve_need",
    keys: "x",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.notNeeded",
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
  {
    // A smart rule that always shows this sender's images (campbooks-cloud#200).
    id: "inbox.trust_images",
    keys: "shift+a",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.trustImages",
  },
  {
    // The smart-rule confirmation: save it (the dialog's own keys are
    // plain; ⌘↵ reaches the registry through it).
    id: "inbox.rule.save",
    keys: "mod+enter",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.saveRule",
  },
  {
    // A taught rule's run: put back everything it changed (teach once).
    id: "inbox.rule.undo_all",
    keys: "shift+z",
    scope: "global",
    group: "item",
    label: "conversations:shortcuts.undoAll",
  },
  {
    // Home's Pay group: every bill it asks about is already paid.
    id: "item.all_paid",
    keys: "shift+e",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.allPaid",
  },
  {
    // Focus moves through the open messages' attachments; Enter opens one.
    id: "inbox.next_attachment",
    keys: "v",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.nextAttachment",
  },
] as const satisfies readonly ShortcutDefinition[];
