# Tasks

## 0. Design gate

- [x] 0.1 Visual prototype v5 approved by the owner (artifact FyiDtRhSG6zr6xfwemYpsU)
- [x] 0.2 Proposal, design, spec deltas (home-ui, inbox-ui, conversations)

## 1. `lib/ui` and `lib/shortcuts`

- [x] 1.1 `Dialog` + `DialogChoices` (focus trap, Esc/scrim, number keys, arrows, bottom sheet, result state); story and tests
- [x] 1.2 `ItemActions` (`ItemAction {id, label, ai?, key}`, roving tabindex, divider); story and tests
- [x] 1.3 `Item`: `actions` slot, `confirmation` + `onConfirmEnd`, bare key hints removed; stories and tests
- [x] 1.4 `Button ai` (visor `xs` before the label); story and test
- [x] 1.5 Toast `pending` and sticky `fail` with Retry/dismiss, existing API kept; stories and tests
- [x] 1.6 Tokens: dialog sizes, `--duration-confirm`, `--duration-result`, `--success-soft`
- [x] 1.7 Shortcuts: `item.snooze` (S), `item.choice3`, `item.next_action`/`item.previous_action`, `item.paid` retired, `conversations.star` on P; app-wide collision test

## 2. Home (`modules/attention`)

- [x] 2.1 Selected row shows its actions; Keyboard aside removed
- [x] 2.2 ↑/↓ + J/K, ←/→ highlight, Enter, Esc, 1-3, E, S, X, R
- [x] 2.3 Snooze dialog with Next week; confirmations that fold; failures through `fail`
- [x] 2.4 Later renamed Snooze in four languages; tests

## 3. Inbox and server

- [x] 3.1 Star renamed Pin in four languages, pin icon
- [x] 3.2 `conversations.list` pinned first with a pinned-aware cursor; indexes; HTTP and unit tests
- [x] 3.3 OpenAPI and client types updated

## 4. Close

- [x] 4.1 Recording scenario `home-actions`
- [x] 4.2 CHANGELOG updated
- [ ] 4.3 Gates green in CI; the lead verifies at 375px and desktop, light and dark, keyboard only
