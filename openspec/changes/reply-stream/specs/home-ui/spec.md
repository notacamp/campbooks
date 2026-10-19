## MODIFIED Requirements

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
Picking an intent, or submitting your own words, SHALL ask for the reply,
sending the likely replies shown with it, and show the draft in an editable
text area, with "Shorter" and "Change…" (a plain-words instruction), both
marked with the visor and rewriting the text as it stands, "Send" (⌘↵),
"Open in composer" and "Back to the choices". On the web the reply SHALL be
asked from `conversations.stream_reply`: while Scout thinks, the dialog
SHALL show a short muted trace of its thinking as it arrives (about five
lines, following the newest, its label announced politely and the text
itself not read aloud), and from the first word it SHALL show the draft
typing in, marked busy, until the whole reply replaces it and the text area
takes focus; the trace SHALL then fold into a closed disclosure. A rewrite
SHALL keep the member's text until its first word arrives and put it back
if the rewrite fails. A new ask, Back, or closing the dialog SHALL stop the
request in flight. The desktop app SHALL ask `conversations.write_reply`
and show the draft when it arrives. No draft SHALL be written before such
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
saying Scout can't write replies on this server; when Scout took too long
(`reply_timed_out`, `request_timed_out`) it SHALL say so and suggest trying
again or writing it yourself; on any other failure it SHALL say so in one
line and keep the text the member had. The model's text and its thinking
SHALL be shown only as text. Below 48rem the dialog SHALL be a bottom
sheet with no horizontal scrolling, in light and dark, in en, pt-PT, es-ES
and fr.

#### Scenario: Pick a likely reply
- **WHEN** the user presses R on Marta's question, then 1
- **THEN** Scout writes "Yes, Thursday 10:00 works" as a reply signed with the user's name, shown editable in the dialog

#### Scenario: Watch Scout write
- **WHEN** the user picks a likely reply on the web and the model thinks before it writes
- **THEN** the dialog shows Scout's thinking in a muted trace, then the reply typing into the draft, and the text area takes focus once the reply is whole

#### Scenario: Combine the suggestions
- **WHEN** the user types "1 and 3, warmer" in the line for their own words
- **THEN** the request carries the three likely replies shown, in order, with that intent

#### Scenario: Make it shorter, then send
- **WHEN** the user chooses Shorter, then presses ⌘↵
- **THEN** the shorter text is sent as a reply to Marta, the dialog says "Sending to Marta", the item folds away, and the toast offers Undo

#### Scenario: A rewrite that fails
- **WHEN** the user chooses Change… and the rewrite fails
- **THEN** the dialog says so and the text is what the user had before

#### Scenario: Close while Scout writes
- **WHEN** the user presses Esc while the reply is still streaming
- **THEN** the dialog closes and the request is stopped

#### Scenario: Scout took too long
- **WHEN** the reply ends with `reply_timed_out`
- **THEN** the dialog says Scout took too long and suggests trying again or writing it yourself

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
