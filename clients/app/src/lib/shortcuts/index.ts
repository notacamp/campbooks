// Public surface of the shortcut system. The engine (one registry of
// definitions + bound handlers feeding the key listener, the `?` sheet, the
// ⌘K menu and every keyboard hint, so hints can't drift from bindings) is
// @notacamp/ui's; Campbooks' catalogue of shortcuts and their labels live here.
export {
  ariaKeyShortcut,
  bindingsOf,
  detectPlatform,
  findConflicts,
  formatBinding,
  handlesArrowKeys,
  isTypingTarget,
  SEQUENCE_TIMEOUT_MS,
  ShortcutRegistry,
  useShortcut,
  useShortcutDefinition,
  useShortcutDefinitions,
  useShortcuts,
  type Platform,
  type ShortcutBindOptions,
  type ShortcutConflict,
  type ShortcutDefinition,
  type ShortcutGroup,
  type ShortcutHandler,
  type ShortcutProviderProps,
  type ShortcutScope,
  type UseShortcutOptions,
} from "@notacamp/ui/shortcuts";
export { DEFAULT_SHORTCUTS, type DefaultShortcutId, type ShortcutId } from "./catalog";
export { ShortcutProvider } from "./ShortcutProvider";
export { shortcutMessages, type ShortcutMessages } from "./messages";
