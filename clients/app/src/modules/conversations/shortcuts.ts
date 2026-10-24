import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Shortcuts for `conversations` operations, keyed by their OpenAPI
 * operationId (ADR 0013), plus the client-only commands of the inbox
 * screens (ids outside the module namespace). Done (E, which moves an
 * archived conversation back to the inbox), Reply (R), J/K, Enter, Z and
 * Escape are the app's defaults; the choices of a suggestion use the
 * defaults' 1 and 2.
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
  // The views beside the streams (campbooks-cloud#240): not streams.
  {
    id: "inbox.view.sent",
    keys: "g 5",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.sent",
  },
  {
    id: "inbox.view.archive",
    keys: "g 6",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.archive",
  },
  {
    id: "inbox.view.spam",
    keys: "g 7",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.spam",
  },
  {
    id: "inbox.view.trash",
    keys: "g 8",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.trash",
  },
  // Pinned and Drafts sit after Sent in the sidebar but take the keys left
  // over, so G 5–8 keep meaning what they always did (campbooks-cloud#277).
  {
    id: "inbox.view.starred",
    keys: "g 9",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.starred",
  },
  {
    id: "inbox.view.drafts",
    keys: "g 0",
    scope: "global",
    group: "navigation",
    label: "conversations:shortcuts.drafts",
  },
  {
    // Delete; in the trash the same key restores it, back where it was.
    id: "conversations.trash",
    keys: "#",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.delete",
  },
  {
    // Report spam; in spam the same key says it is not.
    id: "conversations.report_spam",
    keys: "!",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.reportSpam",
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
  // Scout noticed on Home: its buttons, on Shift + a letter so none
  // meets Home's item keys (R E S H X 1-3 ...).
  {
    // Its own verb: archive them, or file them under a stream.
    id: "inbox.noticed.apply",
    keys: "shift+f",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.noticedApply",
  },
  {
    id: "inbox.noticed.other",
    keys: "shift+o",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.noticedOther",
  },
  {
    id: "inbox.noticed.not_now",
    keys: "shift+l",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.noticedNotNow",
  },
  {
    id: "inbox.noticed.never",
    keys: "shift+d",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.noticedNever",
  },
  {
    // Ask Scout about the suggestion (campbooks-cloud#305).
    id: "inbox.noticed.ask",
    keys: "shift+q",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.noticedAsk",
  },
  {
    // See the mail the suggestion counts: the search for it (campbooks-cloud#347).
    id: "inbox.noticed.see",
    keys: "shift+s",
    scope: "item",
    group: "item",
    label: "conversations:shortcuts.noticedSee",
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
