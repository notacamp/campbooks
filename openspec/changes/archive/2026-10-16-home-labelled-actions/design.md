# Design

## Context

The approved prototype (v5, https://claude.ai/artifact/FyiDtRhSG6zr6xfwemYpsU)
sets the behaviour; this change builds it with the app's components and
tokens and only the actions Home has today. Owner rules: one button style
per row (label and key), follow-up choices in a dialog (Esc = nothing
changed), arrows everywhere, the visor marks AI, every action reports
through the toast plus an in-place confirmation, and the app stays calm.

Data ownership: no new data. `attention` (client) renders Home;
`conversations` owns the list order (schema `conversations`, two new
indexes). No event is added or consumed. The only endpoint touched is
`conversations.list` (`conversations:read`, unchanged).

## Decisions

### 1. An action is `{id, label, ai?, key}`

`lib/ui`'s `ItemActions` takes `own` and `universal` arrays of `ItemAction`
(`key` is a shortcut id from the registry, so hints and `aria-keyshortcuts`
come from one place). Home builds today's own actions from the need's kind
(`rowActionIds` in `attention/model.ts`) and gives them `item.choice1..3` by
position; #166 swaps that for the server's ranked list without touching the
component. A universal action that would repeat an own one is left out
(a question's No is "not needed"; a warning's It's safe is "not needed"),
and Done is not offered on a question until it is answered.

Alternatives: a render prop per button (more freedom, but invites mixed
styles the owner ruled out); variants per action (same problem).

### 2. Focus: roving tabindex, focus follows the highlight

The highlight is state on Home (`highlight`, an action id); `ItemActions`
puts that button (or the first) in the tab order and moves DOM focus to it
when the highlight changes, and a focused button (Tab, click) becomes the
highlight. ←/→ are registry shortcuts (`item.next_action`,
`item.previous_action`) bound only on Home, so the dispatcher keeps its
rules (no arrows in fields or dialogs). Enter is `list.open`: it presses the
highlighted action, or toggles the item. Esc (`overlay.close`, bound only
while something is highlighted or open) clears it and puts the focus back on
the row. When the selection moves and the focus was in the list, the focus
follows to the new row's button, so a screen reader hears where it went.

Rejected: `aria-activedescendant` on a container. The focus normally sits
on the page, not in a composite widget; activedescendant would need a
focused container with `role="toolbar"`, which makes the dispatcher hand
↑/↓ to the widget and breaks moving between items.

### 3. The Dialog primitive

`Dialog` (portal, `role="dialog"`, `aria-modal`, labelled by its title and
described by its description) reuses `useFocusTrap`. Esc and a mousedown on
the scrim call `onClose`; plain keys stop at the dialog so page shortcuts
never fire behind it (⌘/Ctrl combinations still reach the registry, for a
reply's ⌘↵). `DialogChoices` renders choices as buttons marked
`data-dialog-choice`; the dialog maps 1-9 to them and ↑/↓ move between them.
`result` swaps the content for the visor happy, a sentence and a draining
bar (`--duration-result`, 1.1 s), calls `onResultEnd` after it or on any key
or click, and focuses the dialog meanwhile. Below 48rem it is a bottom
sheet. Generic on purpose: title, description, body, footer.

### 4. Confirmations and the toast

`Item` takes `confirmation` and `onConfirmEnd`: it renders a one-line
confirmation that folds (`--duration-confirm`, 0.9 s, no animation under
reduced motion) and calls back after the same time. Home keeps an acted-on
row among its hidden rows with its confirmation until then, and the
selection moves to the next row at once, so E E E works.

`UndoProvider` keeps its API (`push`, `undo`, `toggle`, `notify`,
`history`, so its 12 callers are untouched) and adds `pending({label,
cancel?, cancelled?})` returning `{done, fail}`, and `fail({label,
retry?})`. Pending and failed toasts have no timer; Z cancels a pending
action that allows it; a failure has Retry (when given) and a dismiss
button and is announced as an alert. Home's snooze failures and every need
verdict failure now use `fail` (Retry re-runs it, except a need that changed
meanwhile).

### 5. Keys and names

Later becomes Snooze (`item.snooze`, S); `item.paid` is retired (#167 makes
paid a numbered action); `item.choice3` is added. Star becomes Pin on P in
the UI, but the operation ids stay `conversations.star` / `unstar`: they
are the provider's star and the shortcut id must be the operationId
(ADR 0013). S is also `scout.skip` and P `sign_in.passkey`, both in the
`focus` scope, which never overlaps `item` (one-question pages show no
item); `app/shortcuts.test.ts` asserts the whole app registry has no
conflict and that these are the only sharers.

### 6. Pinned first in the inbox list

`conversations.list` (not archived) orders `starred DESC, last_message_at
DESC, id DESC` with the row-value cursor `(starred, last_message_at, id) <
(…)`; the cursor encodes `pinned|at|id` and a cursor from before (`at|id`)
reads as unpinned. Two partial indexes (per stream and per inbox) match the
order, built concurrently in migrations of their own. Archived lists and
`ids` lists stay newest first. Provider sync is untouched.

## Risks

- Hand-written `.sqlx` entry and OpenAPI/schema descriptions for the list
  query (agents may not build locally); CI's `sqlx prepare --check`,
  `xtask openapi --check` and `api:check` verify them.
- A pinned conversation that was archived elsewhere is not pinned-first in
  the archived list, by design.
