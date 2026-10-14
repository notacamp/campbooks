# Proposal

## Why

The inbox list moves only with J and K. Everyone reaches for ↓ and ↑ first,
and today they scroll the page instead of moving through the conversations.

## What Changes

- A shortcut can declare other bindings next to its keys
  (`alternateKeys`). The dispatcher runs it on any of them, the conflict check
  covers all of them, and a hint can draw them all ("J or ↓"). A screen can
  bind a handler that takes only the main keys, leaving the others to the
  browser.
- List next/previous take ↓/↑ as well as J/K, so the inbox list and Home's
  queue move with the arrows.
- In an open conversation the arrows stay the browser's: they scroll the
  message, while J and K still step between conversations. Moving with the
  arrows never opens (and so never marks read) a conversation; Enter does.
- Arrow keys are never taken from a field, a radio group, a menu, a toolbar, a
  listbox, a grid or an open dialog.
- Moving past the last loaded row asks for the next page, and the row the
  keyboard moves to is scrolled into view in both the list and the reader.
- The empty reader's key list and the `?` sheet show both bindings.

## Impact

- Client only: `lib/shortcuts` (registry, dispatcher, provider), `lib/ui`
  `KbdHint` and `ShortcutSheet`, the inbox screen and reader. No API change.
