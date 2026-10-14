// Public surface of the shortcut system. One registry (definitions + bound
// handlers) feeds the key listener, the `?` sheet, the ⌘K menu and every
// keyboard hint, so hints can't drift from bindings.
export { DEFAULT_SHORTCUTS, type DefaultShortcutId, type ShortcutId } from "./catalog";
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
