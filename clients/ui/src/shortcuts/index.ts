// The keyboard shortcut engine (`@notacamp/ui/shortcuts`). One registry
// (definitions + bound handlers) feeds the key listener, the `?` sheet, the
// ⌘K menu and every keyboard hint, so hints can't drift from bindings. The
// app owns its catalogue of shortcuts; the engine knows only its own few
// (UI_SHORTCUTS).
export { UI_SHORTCUTS, type ShortcutId, type UiShortcutId } from "./catalog";
export { handlesArrowKeys, isTypingTarget, SEQUENCE_TIMEOUT_MS } from "./dispatcher";
export { ariaKeyShortcut, detectPlatform, formatBinding, type Platform } from "./keys";
export {
  bindingsOf,
  findConflicts,
  ShortcutRegistry,
  type ShortcutBindOptions,
  type ShortcutConflict,
  type ShortcutDefinition,
  type ShortcutGroup,
  type ShortcutHandler,
  type ShortcutScope,
} from "./registry";
export {
  ShortcutProvider,
  useShortcut,
  useShortcutDefinition,
  useShortcutDefinitions,
  useShortcuts,
  type ShortcutProviderProps,
  type UseShortcutOptions,
} from "./ShortcutProvider";
export { shortcutMessages, type ShortcutMessages } from "./messages";
