# Design

## One list, two sources

`rowChoices(kind, need.actions)` (attention `model.ts`) returns the row's
own actions as choices `{id, steps}`: one step for an action, two for a pair
(ids joined with `+`). With the key missing it returns `rowActionIds(kind)`
unchanged, so Home without the catalogue renders byte for byte as before.
`null` never comes from the server; the generated type allows it, so it is
read as an empty ranking, which falls back too.

The screen runs a choice by running each step through the same `step()`
switch the fixed actions already used, so Report and delete, It's safe,
Already paid, Still to pay and Reply keep their handlers, toasts and Undo.

## Follow-ups

- **Remind** reuses the Snooze dialog with another title, adding the due-date
  reminder first when a bill has one; choosing calls the same
  `snoozeUntil`, with its confirmation and Undo.
- **Forward** is a dialog in the conversations module (it owns drafts, the
  quote and `list_senders`): a filter box over the people you reply to most,
  asking the server from two characters, as Scout's sender filter does.
  Picking one creates a forward draft addressed to them and opens the
  conversation, where the reader shows the open draft. Sending stays the
  composer's, with its Undo.
- **Open attachment** expands the item and focuses the email's first file:
  the attachment list already previews images and PDFs and saves the rest,
  and the clamp opens when focus lands inside it. No second way to open a
  file.

## The visor

`ItemActions` takes `picked` (its accessible name); the visor goes before
the own actions, not on each button, since `ai` on a button means "runs the
AI" (Reply still carries it).
