# Proposal

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/156

## Why

The inbox still grows past the screen on desktop. `inbox-fits-screen` (#139)
fixed the width; the height was never bounded. The shell has
`min-height: 100dvh`, not a fixed height, and no pane scrolls on its own, so
with a real inbox the list makes the whole page tall: at 1440×900 with 40
conversations the page is 4,073px tall. The sidebar and the open
conversation scroll away with it, and a conversation opened far down the
list shows its reader at the top of the page, out of view. Demo data has
four conversations, so the earlier guard stories never saw it.

## What Changes

- From 64rem the shell is exactly the window's height and its panes never
  wrap. The sidebar, the list and the reader each scroll on their own.
- A pane's header (the stream tabs over the list, the reader's toolbar)
  stays pinned at the top of its pane while the pane scrolls.
- A guard story with 40 conversations checks, at 1100×800 and 1440×900,
  that the page is no taller than the window, that the list scrolls inside
  itself and that the reader stays in view.

## Non-goals

- Below 64rem nothing changes: one pane at a time, and the page scrolls.
- No new layout: the approved two panes stay as they are.

## Impact

- Client only: `lib/ui` Shell (layout CSS) and the inbox screen stories. The
  settings and admin screens use the same shell, so their panes scroll on
  their own from 64rem too. No API, module, event or data change.
