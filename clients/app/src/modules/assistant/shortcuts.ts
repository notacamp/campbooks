import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Keys of the assistant's screens (client-only commands, so their ids are
 * outside the module's namespace). The questions are one-question pages:
 * Enter goes on (wherever the focus is, but a button or a field, which
 * handle Enter themselves), `S` skips one, `B` goes back, Escape is "Not
 * now". Home's offer starts them (`T`) or puts them off (Shift+T); Scout's
 * memory opens them (`Q`) and saves a rule with ⌘↵ while the focus stays in
 * the sentence. `.` opens Scout's chat about the selected need or the open
 * conversation.
 */
export const ASSISTANT_SHORTCUTS = [
  {
    id: "scout.next",
    keys: "enter",
    scope: "focus",
    group: "general",
    label: "assistant:shortcuts.next",
  },
  {
    id: "scout.skip",
    keys: "s",
    scope: "focus",
    group: "general",
    label: "assistant:shortcuts.skip",
  },
  {
    id: "scout.back",
    keys: "b",
    scope: "focus",
    group: "general",
    label: "assistant:shortcuts.back",
  },
  {
    id: "scout.offer.start",
    keys: "t",
    scope: "list",
    group: "general",
    label: "assistant:shortcuts.offerStart",
  },
  {
    id: "scout.offer.not_now",
    keys: "shift+t",
    scope: "list",
    group: "general",
    label: "assistant:shortcuts.offerNotNow",
  },
  {
    id: "scout.questions",
    keys: "q",
    scope: "settings",
    group: "general",
    label: "assistant:shortcuts.questions",
  },
  {
    id: "scout.save_rule",
    keys: "mod+enter",
    scope: "settings",
    group: "general",
    label: "assistant:shortcuts.saveRule",
    allowInInputs: true,
  },
  {
    // Continue with Scout (Home) and Ask Scout (the reader): the selected
    // need's or the open conversation's chat (campbooks-cloud#305).
    id: "scout.ask",
    keys: ".",
    scope: "item",
    group: "item",
    label: "assistant:shortcuts.ask",
  },
] as const satisfies readonly ShortcutDefinition[];
