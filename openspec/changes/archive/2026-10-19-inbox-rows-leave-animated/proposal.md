# Proposal

## Why

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/289

The owner: "When emails leave the list for being archived or marked as spam,
they should do so in an animated way so it's clear what changed in the list."

Today a row that leaves the list on screen (Done, Report spam, Delete, a move
to another stream) disappears in the same frame. The rows below jump up and
the cursor lands on another row. Only the toast at the bottom of the screen
says what happened, far from where the user was looking.

Modules: `conversations` (client only: `clients/app/src/modules/conversations`)
and the design system (`clients/app/src/lib/ui`, `ConversationRow`). No new
module or cross-module dependency.

## What Changes

- A row leaving the list on screen first says what happened to it, in place:
  an icon and "Done", "Moved to Inbox", "Deleted", "Restored", "Reported as
  spam", "Not spam" or "Moved to <stream>", with the subject after it. The line
  wipes in from the left over the row. Report spam and Delete use the danger
  colour; the rest use the success colour.
- Then the row folds away (its height to nothing) and the rows below slide up
  to close the gap. The whole thing takes `--duration-confirm` (900 ms), the
  same timing as a Home row acted on.
- The leaving row is no longer a button: the keyboard cursor skips it, and it
  is hidden from assistive tech, because the toast already says the same.
- Undo while the row folds puts the conversation straight back in the list.
- Under reduced motion the row goes at once, as before.
- `ConversationRow` (ui) gains `leaving` + `onLeft`. The inbox keeps each
  leaving row above the row it sat above, so rows leaving one after another
  (E, E, E) keep their order.

## Non-goals

- No server change.
- Rows that leave because of something else (another device, a sync, the
  Unread filter after reading) still go at once: only the member's own
  actions in this list animate.
- Rows arriving (Undo, new mail) do not animate in.
