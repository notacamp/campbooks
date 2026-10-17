## MODIFIED Requirements

### Requirement: Acting from Home
Enter SHALL open an item in place with its full quote and "How Campbooks
decided" (closed by default) when no action is highlighted. ↑/↓ and J/K
SHALL move between items (closing an open one), R SHALL open the reply
dialog for the item's conversation, E SHALL mark it done
(`conversations.resolve_need`), S SHALL open the Snooze dialog
(`attention.snooze`), X SHALL mark it not needed (a warning's "It's safe",
a question's No), and 1 to 3 SHALL run the item's own actions (a Review
question's Yes and No on 1 and 2). Each action that takes the item off the
list SHALL do so at once, show a toast with Undo, and Z SHALL undo it. The
selection SHALL be an explicit item from the first render; when a live
update removes the selected item, its neighbour at the same position SHALL
take the selection, and E and X SHALL act only on the selected item. When
the server refuses or fails, the item SHALL come back and the toast SHALL
say so and stay until Retry or dismiss (a changed need SHALL be said as
such, with no Retry). The sender's words and name SHALL be isolated from
the page's text direction. Items in an inbox the user may only read SHALL
offer only Snooze: no Reply, Done, Not needed, Yes, No or It's safe, since
each of those writes in the inbox.

#### Scenario: Done and Undo
- **WHEN** the user presses J then E on Pedro's lease, then Z
- **THEN** the item leaves the list with the toast "Done · Pedro Alves", and Z brings it back

#### Scenario: The selected item goes away
- **WHEN** the selected item is cleared elsewhere while Home is open
- **THEN** the item below it is selected, and E acts on that one

#### Scenario: Later
- **WHEN** the user presses S and chooses Monday morning
- **THEN** the item leaves the list and comes back on Monday morning

#### Scenario: Reply from Home
- **WHEN** the user presses R on Marta's question
- **THEN** the reply dialog "Reply to Marta" opens over Home, and the item stays selected behind it

## ADDED Requirements

### Requirement: The reply dialog
Reply (R, a Home item's Reply, the reader's Reply) SHALL open a dialog
titled "Reply to <first name>" that asks `conversations.suggest_replies`
once per message for the session and lists the likely replies as choices
marked with the visor, keys 1 to 3, a line to say it in your own words, and
"Write my own", which SHALL open the plain composer as before. The dialog
SHALL say who the reply goes to, name and address, as the server will send
it (the message's Reply-To first), and SHALL warn when that Reply-To differs
from the sender's address. It SHALL quote the message being answered. While
the suggestions load the dialog SHALL say Scout is reading the conversation.
Picking an intent, or submitting your own words, SHALL call
`conversations.write_reply` and show the draft in an editable text area,
with "Shorter" and "Change…" (a plain-words instruction), both marked with
the visor and rewriting the text as it stands, "Send" (⌘↵), "Open in
composer" and "Back to the choices". No draft SHALL be written before such
a click, and nothing SHALL be sent before Send. Send SHALL create a reply
draft with the text and the quoted history and send it with the existing
undo window: the dialog SHALL show "Sending to <name>" for about a second
and close, a Home item SHALL fold with that confirmation, and the toast
SHALL offer Undo (Z) until the send leaves. An undo in time SHALL bring the item and the dialog back on the same draft on
Home, and the composer on that draft in the reader; sending again SHALL send
that draft, not a new one; a Send or Open in composer SHALL reuse an empty
reply draft of the same message rather than add one, and SHALL never write
over a draft holding the member's words; a failed send SHALL keep the dialog
and the text and say so. "Open in composer" SHALL create the reply draft
with the text and open the composer on it. Esc or a click outside SHALL
close the dialog with nothing created. When the server answers
`reply_unavailable`, the dialog SHALL offer only "Write my own" with a line
saying Scout can't write replies on this server; on any other failure it
SHALL say so in one line and keep the text the member had. The model's
text SHALL be shown only as text. Below 48rem the dialog SHALL be a bottom
sheet with no horizontal scrolling, in light and dark, in en, pt-PT, es-ES
and fr.

#### Scenario: Pick a likely reply
- **WHEN** the user presses R on Marta's question, then 1
- **THEN** Scout writes "Yes, Thursday 10:00 works" as a reply signed with the user's name, shown editable in the dialog

#### Scenario: Make it shorter, then send
- **WHEN** the user chooses Shorter, then presses ⌘↵
- **THEN** the shorter text is sent as a reply to Marta, the dialog says "Sending to Marta", the item folds away, and the toast offers Undo

#### Scenario: Undo the send
- **WHEN** the user presses Z before the send leaves
- **THEN** nothing is sent, the item is back, and the dialog shows the draft again

#### Scenario: Esc changes nothing
- **WHEN** the user opens Reply and presses Esc
- **THEN** the dialog closes and no draft exists

#### Scenario: No model on this server
- **WHEN** the server answers `reply_unavailable`
- **THEN** the dialog says Scout can't write replies here and offers only "Write my own"

#### Scenario: A Reply-To elsewhere
- **WHEN** Marta's message has a Reply-To on another address and the member presses R
- **THEN** the dialog warns that replies go to that address, not to Marta's, before anything is drafted or sent
