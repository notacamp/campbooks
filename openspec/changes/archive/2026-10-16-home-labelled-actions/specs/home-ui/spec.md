## MODIFIED Requirements

### Requirement: Acting from Home
Enter SHALL open an item in place with its full quote and "How Campbooks
decided" (closed by default) when no action is highlighted. ↑/↓ and J/K
SHALL move between items (closing an open one), R SHALL open the
conversation's composer, E SHALL mark it done (`conversations.resolve_need`),
S SHALL open the Snooze dialog (`attention.snooze`), X SHALL mark it not
needed (a warning's "It's safe", a question's No), and 1 to 3 SHALL run the
item's own actions (a Review question's Yes and No on 1 and 2). Each action
that takes the item off the list SHALL do so at once, show a toast with Undo,
and Z SHALL undo it. The selection SHALL be an explicit item from the first
render; when a live update removes the selected item, its neighbour at the
same position SHALL take the selection, and E and X SHALL act only on the
selected item. When the server refuses or fails, the item SHALL come back and
the toast SHALL say so and stay until Retry or dismiss (a changed need SHALL
be said as such, with no Retry). The sender's words and name SHALL be
isolated from the page's text direction. Items in an inbox the user may only
read SHALL offer no Reply, Done or Not needed.

#### Scenario: Done and Undo
- **WHEN** the user presses J then E on Pedro's lease, then Z
- **THEN** the item leaves the list with the toast "Done · Pedro Alves", and Z brings it back

#### Scenario: The selected item goes away
- **WHEN** the selected item is cleared elsewhere while Home is open
- **THEN** the item below it is selected, and E acts on that one

#### Scenario: Later
- **WHEN** the user presses S and chooses Monday morning
- **THEN** the item leaves the list and comes back on Monday morning

### Requirement: Done today
Home SHALL list what left the user's list today (`attention.list` with
`state=cleared`) as plain sentences naming the person and how it left ("You
replied to Marta Ferreira", "Ana marked Pedro Alves's lease done"), with Undo
for Done, Not needed and Snooze, and none for replies, archives and the trash.
On desktop it SHALL sit in the aside, which SHALL hold no keyboard legend
(every key is in the `?` sheet); below 48rem, in a fold after the list.

#### Scenario: Undo from the list
- **WHEN** the user chooses Undo next to "You marked Acme Hosting's bill done"
- **THEN** `conversations.reopen_need` is called and the item is back in Pay

## ADDED Requirements

### Requirement: Labelled actions on the selected item
The selected item SHALL show its actions directly under it, all in one
button style, each its label and its key, in two groups split by a thin
divider: the item's own actions (keys 1 to 3, at most three) and the
universal ones (Done E, Snooze… S, Not needed X). A need offers Reply; a
question (`check`) offers Yes and No and Snooze, with no Done until it is
answered and no Not needed beside its No; a warning offers It's safe, Done
and Snooze. Other rows SHALL show no buttons and no key hints. An action is
`{id, label, ai?, key}` so a ranked list from the server can replace the
own actions. A button for an action that runs the AI SHALL carry the visor
before its label, never a sparkle.

#### Scenario: The selected row
- **WHEN** Marta's reply is selected
- **THEN** under it are "Reply 1", a divider, "Done E", "Snooze… S" and "Not needed X", and no other row shows a button or a key

### Requirement: Arrows along the actions
←/→ SHALL move a visible highlight along the selected row's buttons (from
none, → goes to the first and ← to the last), and the focus SHALL follow it
(one button in the tab order, roving tabindex). Enter SHALL press the
highlighted button; Esc SHALL clear the highlight (and close an open item),
the focus going back to the row. Moving to another item SHALL clear the
highlight.

#### Scenario: Walk and press
- **WHEN** the user selects Marta, presses ← then Enter
- **THEN** Not needed is highlighted and focused, then Marta is marked not needed

### Requirement: Snooze in a dialog
Snooze SHALL open a dialog titled "Snooze <name> until…" with Later today
(while 18:00 is more than an hour away), Tomorrow morning, Monday morning and
Next week, each with its time and its number key, leaving out a day that
repeats an earlier one. Number keys SHALL pick a choice and ↑/↓ move between
them; Esc or a click outside SHALL close it with nothing changed. Below 48rem
it SHALL be a bottom sheet. The dialog is the `lib/ui` Dialog (title,
description, body, footer, a result state that closes itself), which the
reply and smart-rule dialogs reuse.

#### Scenario: Esc changes nothing
- **WHEN** the user presses S, then Esc
- **THEN** the dialog closes, the item is still selected and nothing was snoozed

### Requirement: Confirmations without a click
An item acted on SHALL turn into a one-line confirmation ("Done · Marta
Ferreira", "Snoozed · Pedro Alves, back Mon 08:00") and fold away in under a
second, the next item taking the selection at once; under reduced motion it
SHALL swap with no animation. A dialog may show its result (the visor happy,
a sentence and a thin draining bar) for about a second and close by itself;
any key or click SHALL close it sooner. The bottom toast SHALL stay the
global feedback with Undo and Z, and SHALL offer a pending state (the visor
thinking, Undo while the action can still be taken back) that resolves into
done or failed, and a failure that stays until Retry or dismiss.

#### Scenario: A failed snooze
- **WHEN** the server refuses a snooze
- **THEN** the item is back in the list and the toast says it could not be snoozed until the user chooses Retry or dismisses it
