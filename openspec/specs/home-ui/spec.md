# home-ui Specification

## Purpose
A workspace's Home once mail is connected: what needs the member, grouped by
reason and due soonest first, with keyboard actions, undo, Done today and an
honest empty state.

## Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups, in the order Review, Reply,
Decide, Do, Pay, leaving out empty groups, each item in the list's order. The
hero SHALL say how many things need the user, with the Visor `alert` when
Review has an item. The selected item SHALL be shown by the selected background and a full focus
ring, never by a coloured stripe on one edge. Each item SHALL show who it is from, its sentence (from a
template chosen by reason and by the rule that decided it, in the user's
language), the sender's sentence quoted on one line, and its chips (due date
or overdue, urgent, how long it has waited). Rows SHALL come from the owning
module's batch read (`conversations.list` with `ids`), not from `attention`.

#### Scenario: A mixed morning
- **WHEN** the user has a phishing warning, two replies, a dated ask and a bill
- **THEN** Home says "5 things need you", Review comes first with the warning, then Reply, Do and Pay, and nothing shows a percentage

#### Scenario: Due soonest first
- **WHEN** the Reply group holds a question due Wednesday and an older one with no date
- **THEN** the one due Wednesday is first

### Requirement: Acting from Home
Enter SHALL open an item in place when no action is highlighted, showing the
email the need is about as the reader renders it (sanitised, in the sandboxed
frame, remote images held back, its links leading nowhere when the need is a
warning), a few lines deep with "Show the whole email" to read it in place,
and "How Campbooks decided" (closed by default); the sender's quote SHALL be
shown only when there is no email to show. ↑/↓ and J/K SHALL move between
items (closing an open one), R SHALL open the reply dialog for the item's
conversation, E SHALL mark it done (`conversations.resolve_need`), S SHALL
open the Snooze dialog (`attention.snooze`), X SHALL mark it not needed (a
warning's "It's safe", a question's No), and 1 to 3 SHALL run the item's own
actions (a Review question's Yes and No on 1 and 2). Each action that takes
the item off the list SHALL do so at once, show a toast with Undo, and Z
SHALL undo it. The selection SHALL be an explicit item from the first
render; when a live update removes the selected item, its neighbour at the
same position SHALL take the selection, and E and X SHALL act only on the
selected item. When the server refuses or fails, the item SHALL come back
and the toast SHALL say so and stay until Retry or dismiss (a changed need
SHALL be said as such, with no Retry). The sender's words and name SHALL be
isolated from the page's text direction. Items in an inbox the user may only
read SHALL offer only Snooze: no Reply, Done, Not needed, Yes, No or It's
safe, since each of those writes in the inbox.

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

#### Scenario: Reading the email before deciding
- **WHEN** the user selects Marta's item and presses Enter
- **THEN** her email shows under the item a few lines deep, and "Show the whole email" shows all of it in place

#### Scenario: A warning's links lead nowhere
- **WHEN** the user opens a phishing warning on Home
- **THEN** its email shows with links that open nothing, and a line says links are off in mail that may be phishing

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

### Requirement: Nothing needs you
When the user can see an inbox and has no open item, Home SHALL show the
Visor happy, "Nothing needs you", a line saying how many messages Campbooks
read today, and Done today.

#### Scenario: All clear
- **WHEN** the last item is marked done
- **THEN** Home shows "Nothing needs you" and the item in Done today

### Requirement: Home stays current
Home SHALL refetch its items when the change feed brings an `attention.item`
change and when the earliest snooze it knows of ends, and SHALL show the same
groups, actions and states at 375px (no horizontal scrolling, the tab bar's
Home badge with the count) and on desktop, in light and dark, in en, pt-PT,
es-ES and fr, passing axe.

#### Scenario: Mail arrives while Home is open
- **WHEN** a new question reaches the user's inbox while Home is open
- **THEN** it appears in Reply without a reload

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

### Requirement: Is it paid?
A Pay need that is a question (trust `check`, question `money`) SHALL read
"<Payee> sent an invoice. Is it paid?" (no amount until one is read from the
invoice; never parsed from the mail's words) and offer Already paid (1) and
Still to pay (2), then Snooze (S) and Not needed (X). Already paid SHALL fold
the row into "Marked paid · <payee>" with Undo in the toast. Still to pay
SHALL keep the row in Pay as "Pay <payee>", with "N days overdue" or its due
date when the need has one (from `due_on` only), and offer first a reminder:
two days before the due date at 09:00, tomorrow at 09:00 once that has
passed or the bill is overdue, or next Monday at 09:00 with no due date,
labelled by the time it gives ("Remind me tomorrow at 09:00", "Remind me on
28 October"); then Mark paid (2), Snooze and Not needed. The reminder SHALL snooze the item
until then, with the Snooze toast and Undo. When the Pay group holds two or
more bills asked about, its heading SHALL offer "All N already paid" (Shift+E),
which marks them all paid with one toast and one Undo for all of them (when
some fail, the Undo covers the others and one failure offers Retry for the
rest); the group
SHALL say that Campbooks can't see the bank, so it asks. After Already paid
the screen SHALL leave a hook for offering a paid rule for that sender
(campbooks-cloud#169), which offers nothing yet.

#### Scenario: Already paid
- **WHEN** Sam selects "Acme Hosting sent an invoice. Is it paid?" and presses 1
- **THEN** the server records `already_paid` for the message Sam saw and the row folds into "Marked paid · Acme Hosting"

#### Scenario: Still to pay
- **WHEN** Sam presses 2 on the same bill, due 30 October, on 13 October
- **THEN** the row reads "Pay Acme Hosting" and its first action is a reminder for 28 October at 09:00

#### Scenario: Overdue
- **WHEN** a bill Sam said is still to pay was due yesterday
- **THEN** its row says "1 day overdue" and its reminder is tomorrow at 09:00

#### Scenario: All already paid
- **WHEN** the Pay group asks about two bills and Sam chooses "All 2 already paid", then Z
- **THEN** both are marked paid with one toast "Marked 2 invoices paid", and Z reopens both

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
