# Proposal

## Why

Home's quick-actions redesign (epic campbooks-cloud#164, prototype v5,
approved by the owner) starts with the foundation every later piece uses:
an item says what you can do with it, in words, right where you look, and
every action reports back without a click. Today the actions only appear
once a row is opened, in four different button styles, the keys sit bare
on the row and in an aside legend, "Later" lives in a small popover, and a
failure toast fades after six seconds whether you saw it or not.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/165
(part of #164; the reply dialog is #168, smart-rule confirmations #169,
ranked server actions #166, paid as a numbered action #167).

## What Changes

- **Labelled actions on the selected row.** The selected item shows its
  actions directly under it, in one button style (label plus key), in two
  groups split by a thin divider: the item's own actions (keys 1 to 3)
  and the universal ones (Done E, Snooze… S, Not needed X). Other rows
  stay calm. The bare key hints leave the rows and the Keyboard section
  leaves Home's aside; every key stays in the `?` sheet. The item's own
  actions are today's (Reply; Yes and No for a question; It's safe for a
  warning), shaped as `{id, label, ai?, key}` so #166's ranked list drops
  in.
- **Arrows everywhere.** ↑/↓ move between items (J/K stay), ←/→ move a
  visible highlight along the selected row's buttons, Enter presses it
  (or opens the item when nothing is highlighted), Esc clears it.
- **A Dialog primitive** in `lib/ui`: modal, focus trapped, Esc closes
  with nothing changed, a click on the scrim closes, number keys pick a
  choice, arrows move between choices, a bottom sheet below 48rem, and a
  result state (visor happy, a sentence and a thin draining bar) that
  closes by itself after about a second or at any key. Snooze uses it:
  Later today, Tomorrow, Monday, and a new Next week.
- **Confirmations without a click.** A row acted on becomes a one-line
  confirmation ("✓ Done · Marta Ferreira") and folds away in under a
  second, the next row taking the selection (an instant swap under
  reduced motion). The toast keeps Undo and Z, and gains a **pending**
  state (the visor thinking, Undo while the action allows it) that
  resolves into done or failed, and a **failure** that stays until Retry
  or dismiss.
- **Names and keys.** Later becomes **Snooze** (`item.snooze`, S) in the
  UI and all four languages. Star becomes **Pin** (P) in the UI;
  `item.paid` is retired (paid becomes a numbered action in #167).
  **Pinned conversations stay at the top of their stream** in the inbox
  list (`conversations.list` orders pinned first). Provider star sync is
  unchanged: Pin is the same `starred` flag, Gmail's STARRED.
- **The AI mark.** `Button` takes `ai` to put the small visor before its
  label, for actions that run the AI (Reply in #168).

Modules touched: `attention` (client Home), `conversations` (server list
ordering; client inbox names and keys), `assistant`/`identity` (key
collision tests only), `lib/ui`, `lib/shortcuts`.

## Non-goals

- No ranked or model-chosen actions (#166), no reply dialog or drafting
  (#168), no smart-rule offers (#169), no paid answers (#167).
- No new endpoint, scope, event or migration. The `conversations.star`
  and `conversations.unstar` operation ids stay (they are the provider's
  star), only their UI name changes.
- No change to how the inbox's own reader is laid out beyond the names
  and keys.
