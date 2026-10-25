# home-ui Specification

## Purpose
A workspace's Home once mail is connected: what needs the member, grouped by
reason and due soonest first, with keyboard actions, undo, Done today and an
honest empty state.

## Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups by priority band, in the order
Now, Today, This week, Whenever, leaving out empty groups, each item in the
list's order. Each group SHALL be headed by its name and its count, Now in
the danger tone. The hero SHALL say how many things need the user, with the
Visor `alert` when a Review item is on the list. The selected item SHALL be shown
by the selected background and a full focus ring, never by a coloured stripe
on one edge. Each item SHALL show, before its name, an eyebrow line with its
kind and why it is in its band (see *The reason first*), then who it is from,
its sentence (from a template chosen by reason and by the rule that decided
it, in the user's language), the sender's sentence quoted on one line, and,
at the row's end, how long it has waited. The age SHALL count from when the
item's cause arrived (`since`), in the user's language: minutes or hours the
same day, "Yesterday" and days ago in the week after, the date from a week on
(with the year when it is another year's), with the full date and time on
hover; every row SHALL show it, dated or not. Rows SHALL come from the owning
module's batch read (`conversations.list` with `ids`), not from `attention`.
The keys (↑/↓ and the rest) SHALL follow the order on screen.

#### Scenario: A mixed morning
- **WHEN** the user has a phishing warning, a bill overdue by a day, a reply due tomorrow, a dated ask due Friday and a reply with no date
- **THEN** Home says "5 things need you", Now holds the warning and the bill, Today the reply due tomorrow, This week the ask, and nothing shows a percentage

#### Scenario: Due soonest first
- **WHEN** This week holds a question due Wednesday and an older one with no date
- **THEN** the one due Wednesday is first, and its eyebrow says "Due Wed"

#### Scenario: How long it has waited
- **WHEN** Marta's question arrived at 10:00 today, Pedro's on Monday and the bill on 2 October, and it is Friday 15:00
- **THEN** Marta's row says "5 hours ago", Pedro's "4 days ago" and the bill's "2 Oct", each with its full date on hover, and the bill's due date is in its eyebrow

#### Scenario: A nudge after the bills
- **WHEN** the user has a bill overdue by a day, a nudge and a reply from a service with no date
- **THEN** Home says "3 things need you" and lists the bill in Now, the nudge in Today, then the reply in Whenever

#### Scenario: Down the bands with the keys
- **WHEN** the user presses ↓ on the last item of Now
- **THEN** the first item of Today is selected

### Requirement: Acting from Home
Enter SHALL open an item in place when no action is highlighted, showing
Scout's summary of the email first when there is one (under "Scout's summary",
marked with the visor, as text only; nothing when no summary is written, no
model may read mail here or the call failed), then the email the need is
about as the reader renders it (sanitised, in the sandboxed
frame, remote images held back, its links leading nowhere when the need is a
warning), a few lines deep with "Show the whole email" to read it in place,
and "How Campbooks decided" (closed by default); the sender's quote SHALL be
shown only when there is no email to show. ↑/↓ and J/K SHALL move between
items and open the item moved onto, as a click on it does (Enter or Esc
closes it again); when an open item leaves the list, the item taking its
place SHALL open, so reading goes on item by item. R SHALL open the reply dialog for the item's
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

#### Scenario: Reading item by item with the keys
- **WHEN** the user presses J on Home, then E
- **THEN** J selects and opens Marta's item, showing her email as a click would, and after E the item below opens in her place

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

#### Scenario: Scout's summary first
- **WHEN** Marta's need has a summary and the user opens her item
- **THEN** "Scout's summary" with the visor shows what the email is about, above the email, and nothing is asked of the model

#### Scenario: No summary
- **WHEN** an item's need has no summary
- **THEN** the opened item shows the email without any summary or placeholder

#### Scenario: A warning's links lead nowhere
- **WHEN** the user opens a phishing warning on Home
- **THEN** its email shows with links that open nothing, a line says links are off in mail that may be phishing, and there is no Scout's summary

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
`{id, label, ai?, key}`; a need the server ranked offers its own actions
instead (Scout's ranked actions). A button for an action that runs the AI SHALL carry the visor
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
SHALL keep the row as "Pay <payee>", with "N days overdue" or its due date
when the need has one (from `due_on` only), and offer first a reminder:
two days before the due date at 09:00, tomorrow at 09:00 once that has
passed or the bill is overdue, or next Monday at 09:00 with no due date,
labelled by the time it gives ("Remind me tomorrow at 09:00", "Remind me on
28 October"); then Mark paid (2), Snooze and Not needed. The reminder SHALL snooze the item
until then, with the Snooze toast and Undo. When Home holds two or more
bills asked about, wherever their bands put them, a line under the list SHALL
offer "All N already paid" (Shift+E), which marks them all paid with one
toast and one Undo for all of them (when some fail, the Undo covers the
others and one failure offers Retry for the rest). The opened item of a bill
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
- **WHEN** Home asks about two bills, one in Now and one in Whenever, and Sam chooses "All 2 already paid" under the list, then Z
- **THEN** both are marked paid with one toast "Marked 2 invoices paid", and Z reopens both

#### Scenario: Why it asks
- **WHEN** Sam opens a bill Campbooks asks about
- **THEN** the opened item says Campbooks can't see the bank, so it asks whether each invoice is paid

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

### Requirement: Report and delete
A warning on Home SHALL offer "Report and delete" (1) before "It's safe":
`conversations.report_spam`, which moves the conversation to spam at the
provider, with the row's one-line confirmation and a toast whose Undo puts it
back where it was (`conversations.not_spam`). Once the server agreed, Home
SHALL ask `conversations.suggest_rules` with `action=report` and, when there
is an offer, show it docked above the toasts: "Never see emails like this
again?", what it would do in words (mail that says it is from the domain but
fails its sender checks, or the sender), "Yes, from now on" (1) and "Just
this one" (Esc, which dismisses the offer). Yes SHALL open the smart-rule
confirmation and never save on its own.

#### Scenario: A fake PayPal
- **WHEN** the member reports a phishing message from service@paypal.com that failed its checks
- **THEN** it leaves Home with Undo, and the offer asks "Never see emails like this again?" for "mail that says it's from paypal.com but fails its sender checks"

### Requirement: The paid rule offer
After Already paid on a bill, once the server agreed, Home SHALL ask
`conversations.suggest_rules` with `action=paid` and, when there is an
offer, show "Are <payee>'s invoices always paid for you?" with "Yes, from now
on" (1) and "Just this one" (Esc); Yes SHALL open the smart-rule confirmation
for "mark <payee>'s invoices paid". "All N already paid" offers nothing.

#### Scenario: EDP
- **WHEN** the member says EDP's invoice is already paid
- **THEN** the offer asks whether EDP's invoices are always paid, and Yes opens the confirmation of the paid rule

### Requirement: Scout noticed
Right after the first group of Home (and where the groups would be when
nothing needs the member), Home SHALL show at most one suggestion from
`conversations.list_rule_notices`: a title line with "Scout noticed" and "n
of N", then what Scout noticed in a sentence built from the counts in the
member's language ("Design Notes sent you 88 emails in the last 60 days, and
you archived or deleted 88 of them unread. Archive them as they arrive?"), set
in the body font for reading, then a link "See the N emails" that opens the
search results for the sender's mail in the window Scout counted
(`from:<sender> newer_than:<days>d`), then one row of buttons in one style:
the suggestion's own verb ("Archive them…", or "File under Updates…"),
"Something else…" (with the visor before it) and "Not now", and a More menu
holding "Don't suggest this" and "Ask Scout" (where a chat is mounted). "Not
now" and "Don't suggest this" SHALL each show a toast with Undo, and the next
suggestion shows. Both of the first two SHALL open the smart-rule
confirmation, which asks what should happen to the sender's mail first. The
button row SHALL never wrap: where it does not fit (a phone), its buttons fold
into More with the rest, each with its name and key. In a shared inbox it
SHALL say the rule would act for everyone in that inbox. It SHALL not show
while Home loads or the list fails or is empty, and at 375px as on desktop it
sits after the first group.

#### Scenario: Two suggestions
- **WHEN** Scout noticed build notifications and job alerts
- **THEN** Home shows "1 of 2" with the build notifications; Not now shows "1 of 1" with the job alerts, and Undo brings the first back

#### Scenario: Higher on Home
- **WHEN** Home has Now, Today and Whenever groups and Scout noticed something
- **THEN** the note sits between the Now group and the Today group

#### Scenario: Nothing needs the member
- **WHEN** nothing needs the member and Scout noticed something
- **THEN** the note shows under the hero, where the groups would be

#### Scenario: Readable
- **WHEN** Scout noticed CI's build notifications
- **THEN** "Scout noticed" and "1 of 1" make a title line, and the sentence under it is in the body font

#### Scenario: See the emails
- **WHEN** Ana presses "See the 34 emails" on the suggestion about builds@ci.example.com counted over 60 days
- **THEN** the search results open for `from:builds@ci.example.com newer_than:60d`

#### Scenario: One row
- **WHEN** the suggestion shows on a desktop
- **THEN** the verb, Something else… and Not now sit on one row with More after them, and Don't suggest this and Ask Scout are in More with their keys

#### Scenario: One row on a phone
- **WHEN** the suggestion shows at 375px
- **THEN** the row holds the verb and More, and More lists Something else…, Not now, Don't suggest this and Ask Scout

### Requirement: Scout's ranked actions
A need that carries `actions` (the catalogue is on and its message was
rated) SHALL offer them as its own actions, in the server's order, on keys 1
to 3, in the same row, with the visor before them saying Scout picked them
(never a sparkle); the universal actions SHALL stay as they are. A need
without the `actions` key SHALL offer exactly the fixed actions above; only
a missing key falls back. A question (`check`) SHALL keep Yes and No, a
bill already said to be still to pay SHALL never offer Still to pay again,
and a bill's Already paid and Still to pay, a bill still to pay's Mark paid
and a warning's Report and delete and It's safe SHALL always be there, first
when the ranking left them out; a warning SHALL offer only Report and delete
and It's safe, whatever the ranking holds. Reply, Report and delete, It's safe, Already paid and Still to pay SHALL do
what the fixed actions do, with the same confirmation and Undo. Remind SHALL
open the Snooze dialog as "Remind me about <name>…", a bill's reminder
before its due date first, and snooze it with Undo. Forward SHALL open a
dialog that finds people by name (the people replied to most before
anything is typed, the server's matches from two characters), never an
address typed from memory, only among the people it is safe to write to
(`list_senders` with `reachable`), so a name typed for a sender under review
or a no-reply address finds nobody, with no warning, and picking one SHALL
open the conversation with a forward draft addressed to them; nothing is
sent from Home. A forward of
that email nobody wrote in yet (no text of the member's own, no copies, no
files, at most one person) SHALL be addressed again rather than doubled; any
other SHALL stay as it was and a new one start. When the email or its drafts
can't be read, the dialog SHALL say so and let another pick try, and a start
still running SHALL then open nothing; closing it while the forward starts
SHALL go nowhere. Either way the draft it made SHALL be known at once, so
reopening Forward addresses it again rather than starting a second one. Open
attachment SHALL open the item with its email's first file focused, and say
so when there is no file to open ("This email has no file to open"). A pair
the server offers as one SHALL be one button that runs both, in order. An
inbox the member may only read SHALL offer only Remind and Open attachment
among them. A pair of two actions that ask a follow-up
in a dialog SHALL never be offered, and Open attachment before a dialog only
opens the item.

#### Scenario: Ranked
- **WHEN** the phishing warning carries It's safe (81) then Report and delete (19)
- **THEN** its own actions are "It's safe 1" then "Report and delete 2", the visor before them, and 1 marks it safe

#### Scenario: Not rated
- **WHEN** a need has no `actions` key
- **THEN** it offers exactly the fixed actions, with no visor before them

#### Scenario: Forward by name
- **WHEN** the user presses Forward on Marta's need and types "joao"
- **THEN** only João Matos is listed, and picking him opens Marta's conversation with a forward draft to him

#### Scenario: A pair
- **WHEN** a need carries the pair Open attachment and Forward
- **THEN** it shows one button, "Open attachment, then Forward…", that does both

#### Scenario: Not to a suspect
- **WHEN** the user presses Forward and types "support" or "accounts"
- **THEN** neither the phishing IT Support nor no-reply@accounts.example is offered, and the dialog says no one by that name has written

#### Scenario: Reopened after Esc
- **WHEN** the user picks Rita, presses Esc while the forward starts, then forwards again to João
- **THEN** the same draft is addressed to João; no second draft is made

### Requirement: A reply answers the message its draft was written for
Once a draft is asked for (a likely reply, your own words) or handed back
(an undo), the reply dialog SHALL keep answering that message: newer mail
arriving in the conversation SHALL never change what Send, Open in composer
or the quote answer. When that newer message is received mail (not the
member's own), the dialog SHALL then say "A newer message from <name>
arrived", with "Answer the newer one", which SHALL write the draft again for
the newer message (or, with no intent, quote it instead), never reusing a
draft saved for the older one. A draft already saved (a send that failed)
SHALL never be reused after Back either: the next save SHALL be a fresh
draft, and the saved one, still editable, SHALL be discarded. Open in composer SHALL open the composer on
the message the draft answers. When a send has no send time (no window to
take it back in), the toast SHALL offer no Undo. A reader without a
composer SHALL offer no Reply, Reply all or Forward, and R SHALL do nothing
there.

#### Scenario: Mail arrives while the draft is open
- **WHEN** the user has Scout's draft for Maya's message open and Leo's answer arrives in the conversation
- **THEN** the dialog says a newer message from Leo Brandt arrived, and Send replies to Maya's message with Maya's quoted

#### Scenario: Answer the newer one
- **WHEN** the user then chooses "Answer the newer one"
- **THEN** Scout writes the draft again for Leo's message, the quote is Leo's, and Send replies to it

#### Scenario: No window, no Undo
- **WHEN** a send comes back with no send time
- **THEN** the toast says "Sending to Maya Okafor" with no Undo

#### Scenario: Back after a failed send
- **WHEN** a send fails, and the user goes Back to the choices, picks another and sends
- **THEN** a fresh draft is sent, and the draft of the failed send is discarded

#### Scenario: Your own mail is not a newer message to answer
- **WHEN** the member's own sent mail lands in the conversation while the draft is open
- **THEN** the dialog says nothing of a newer message

### Requirement: Dialog keys stay in the dialog
A dialog SHALL keep the focus inside it: when its focused control goes away
(a step replaces it) or the focus leaves for nowhere, the focus SHALL return
to the dialog, so Esc, its number keys and arrows keep working and plain
keys never reach the page behind it. It SHALL listen on itself only, never
on the whole document: a key the page itself receives SHALL be the page's.

#### Scenario: A step replaces the focused control
- **WHEN** the focused button of a dialog is replaced by choices, and the user presses 2
- **THEN** the second choice is picked, and Esc closes the dialog

### Requirement: Still needed?
An item whose need is a still-needed question SHALL be shown in Review as a
question naming the sender and how long ago they wrote, in the user's
language: "Pedro Alves asked 3 weeks ago. Still to do?" for `still_to_do`,
"Pedro Alves wrote 3 weeks ago. Still needs a reply?" for
`still_needs_reply` (the plain question "Did … ask you to do something?" or
"Does … need a reply?" when the sent time is unknown). Its Yes (1) SHALL call
`conversations.confirm_need` and its No (2, X) `conversations.resolve_need`
with `not_needed`; both take the item off the list with Undo, and No SHALL
neither say Campbooks will learn from it nor offer to teach a rule. An item
whose ask lapsed SHALL not be shown.

#### Scenario: An old lease
- **WHEN** Pedro Alves's undated request to sign a lease is three weeks old
- **THEN** Home shows it under Review as "Pedro Alves asked 3 weeks ago. Still to do?" with Yes and No

#### Scenario: Yes
- **WHEN** the user presses 1 on it
- **THEN** the item leaves Review with the toast "Thanks · Pedro Alves stays on your list", Z brings the question back, and it returns under Do as the user's own

#### Scenario: No longer needed
- **WHEN** the user presses 2 on it
- **THEN** the item leaves the list with the toast "No longer needed · Pedro Alves" and no rule is offered

### Requirement: Worth a look
Home SHALL show mail a rule the member taught with "Highlight them" in the
Whenever band, its eyebrow saying "Worth a look" with a signal colour of its
own (the `signal-look` token) and "Kept here by your rule": one item per
conversation, saying "You asked to see mail from <name>". Its row SHALL offer
Seen (Done, E) and Snooze (S) only: no Reply, and no Not needed, since nothing
is asked of the member. Seen SHALL fold the row with "Seen · <name>" and Undo
like Done. The item SHALL leave Home by itself once the message is opened, and
Done today SHALL not list that.

#### Scenario: Seen
- **WHEN** Ana presses E on a Worth a look item
- **THEN** it folds into "Seen · Design Notes" with Undo, and the conversation is not archived

#### Scenario: Opened elsewhere
- **WHEN** Ana opens the highlighted conversation from the inbox
- **THEN** its item leaves Home, and Done today does not list it

### Requirement: Deleting the draft from the Reply dialog
In its draft stage the Reply dialog SHALL offer "Delete draft" once a draft
exists: when it opened on a draft whose send was undone, or after Open in
composer saved one. It SHALL NOT be offered before anything is saved. It
SHALL delete that draft, close the dialog and show "Draft deleted" with
Undo, which recreates the draft with the text that was on screen. A failed
delete SHALL keep the dialog and the draft and say so.

#### Scenario: Delete a resumed draft
- **WHEN** the dialog opens on a draft whose send was undone and the user chooses Delete draft
- **THEN** the draft is deleted, the dialog closes and the toast offers Undo

#### Scenario: Nothing to delete yet
- **WHEN** the user has picked one of Scout's replies but not saved a draft
- **THEN** the dialog does not offer Delete draft

### Requirement: Worth a look is not counted as needing the member
Worth a look items SHALL NOT count in the hero's "N things need you" nor in the
Home badge (the sidebar and the tab bar), since they ask nothing of the member;
the Whenever group's count SHALL include them. When the only items are Worth a
look ones, the hero SHALL say "Nothing needs you" with the subline "Nothing
asks anything of you. N emails below are worth a look." (singular for one), in
the member's language, and the group SHALL show below it.

#### Scenario: A mixed Home
- **WHEN** Home holds six items that need the member and one Worth a look, alone in Whenever
- **THEN** the hero says "6 things need you", the badge says 6, and the group reads "Whenever 1"

#### Scenario: Only Worth a look
- **WHEN** Home's only item is a Worth a look one
- **THEN** the hero says "Nothing needs you" and "Nothing asks anything of you. 1 email below is worth a look.", the item is shown, and the badge is empty

### Requirement: Scout noticed keys
Scout noticed's actions SHALL show their keys and work from the keyboard while
no confirmation is open, whether they show as buttons or in More: Shift+F (its
own verb), Shift+O (Something else…), Shift+L (Not now), Shift+D (Don't
suggest this), Shift+Q (Ask Scout) and Shift+S (See the N emails). None SHALL
share keys with Home's item keys or any other shortcut.

#### Scenario: Not now by key
- **WHEN** Ana presses Shift+L with a suggestion showing
- **THEN** it is hidden with the usual Undo, as if she had pressed Not now

#### Scenario: Don't suggest this by key, from More
- **WHEN** Ana presses Shift+D while Don't suggest this is folded in More
- **THEN** the suggestion is hidden with the usual Undo, without opening More

#### Scenario: The emails by key
- **WHEN** Ana presses Shift+S with a suggestion showing
- **THEN** the search results for the sender's counted mail open

### Requirement: Telling Scout about the person in the reply dialog
Under the quoted message, the reply dialog SHALL show one quiet line about
the person answered, found as the server finds them (the sender of a
message received, the first recipient of one sent): "Tell Scout about
<first name>" (marked with the visor) when the member has no note about
their address, or "Scout knows: <the note, on one line, cut with an
ellipsis>" with "Edit" when they have one. The line SHALL be hidden when the
address is not plain, while the member's notes load or fail to load, and
when the server has no model for replies (`reply_unavailable`), since the
note only steers Scout. The client SHALL NOT try to tell a distrusted
conversation apart: the server gives a distrusted message no note.

Activating the line SHALL open, in the dialog and never as a dialog of its
own, one field labelled "About <name>, and how to write to them" with the
placeholder "e.g. My landlord. Formal, in Portuguese.", at most 500
characters with a counter near the limit, Save (⌘↵ / Ctrl+Enter, shown as
the app shows its hints) and Cancel (Esc), and, for a note kept, "Forget
this". ⌘↵ and Esc in the field SHALL save and cancel the note only: they
SHALL never send the reply or close the dialog. Save SHALL call
`conversations.set_person_note` with the address in the body, show the new
line, put the focus back on it, announce "Saved" in a live region, and say
so in a toast with Undo that puts back what was there before. Forget this
SHALL call `conversations.delete_person_note`, with Undo. Once a note is
saved while a draft shows, the dialog SHALL offer "Rewrite with this", which
asks for the reply again with the same intent, as a pick does
(`conversations.stream_reply` on the web, `conversations.write_reply` on the
desktop); it SHALL never rewrite on its own. A note that can't be kept SHALL say why under the field
(`person_notes_full`, `person_note_invalid`, or that it failed) and keep the
words. In en, pt-PT, es-ES and fr, and at 375px with no horizontal
scrolling.

#### Scenario: Tell Scout about Marta
- **WHEN** Sam opens Reply on Marta's email, chooses "Tell Scout about Marta", types "My landlord. Formal, in Portuguese." and presses ⌘↵
- **THEN** the note is kept for marta's address, the line reads "Scout knows: My landlord. Formal, in Portuguese." with Edit, the focus is on it, and nothing is sent

#### Scenario: Rewrite with this
- **WHEN** a draft shows and Sam saves a note about Marta
- **THEN** "Rewrite with this" is offered, and Scout writes the draft again only when Sam chooses it

#### Scenario: Esc keeps the dialog
- **WHEN** Sam presses Esc in the note's field
- **THEN** the field closes with nothing kept, the dialog stays open, and the focus is back on the line

#### Scenario: No model on this server
- **WHEN** the server answers `reply_unavailable`
- **THEN** the dialog shows no line about the person

### Requirement: Nudge items
A `nudge` item SHALL say who has not written yet, by the awaited person's
name ("Ana Costa hasn't written yet") when someone else on the email was
awaited, or "No word from Rui Matos yet" when the sender was, with the
sender's sentence quoted on one line, its eyebrow saying when an answer was
due ("Ana's answer was due Mon") and, at the row's end, since when it has
waited ("Asked 14 Oct"). The opened item SHALL say that Scout drafts the
nudge and the member sends it, and, after a nudge, how many were sent
("Nudged once"). Its actions SHALL be Nudge (1, the nudge dialog), Heard
from them (E, `conversations.resolve_need`), Later (S) and Not waiting (X,
the need cleared as not needed, a correction when its trust is `check`),
each with the row's confirmation and Undo (Z). An item in an inbox the user
may only read SHALL offer only Later.

#### Scenario: Ana hasn't written
- **WHEN** the wait on Ana Costa in João's conversation turns into a nudge on Monday
- **THEN** Home lists, in Today, "Ana Costa hasn't written yet", João's line quoted, "Asked 14 Oct", with Nudge on 1

#### Scenario: Not waiting after all
- **WHEN** the user presses X on it
- **THEN** it leaves the list with the toast "Not waiting on Ana Costa" and Undo

### Requirement: The nudge dialog
Nudge SHALL open a dialog titled "Nudge <first name>" that says who the
nudge goes to and who is copied, as the server will address it (the `nudge`
draft kind), quotes the line it follows up on, says Scout is writing while it
waits for `conversations.write_nudge`, and then shows the text in an editable
area with "Shorter" and "Change…" (marked with the visor), "Send" (⌘↵),
"Open in composer" and "Write my own". Nothing SHALL be drafted before the
dialog opens it and nothing SENT before Send. Send SHALL send with the usual
undo window, say "Sending to <name>", fold the item away and toast "Nudged
<name> · Scout checks again <day>" with Undo (Z) until the send leaves. The
reply dialog's rules for reusing an empty draft, never writing over the
member's words, failures and Esc SHALL apply. When the server answers
`reply_unavailable`, the dialog SHALL offer only "Write my own" (the
addressed `nudge` draft in the composer) with a line saying Scout can't write
here. Below 48rem it SHALL be a bottom sheet with no horizontal scrolling, in
light and dark, in en, pt-PT, es-ES and fr.

#### Scenario: Nudge and send
- **WHEN** the user presses 1 on Ana's item, then ⌘↵
- **THEN** the dialog shows "To Ana Costa · Copying João Matos" and Scout's few lines, the nudge is sent, the item folds away and the toast says "Nudged Ana · Scout checks again Thursday"

#### Scenario: Undo the nudge
- **WHEN** the user presses Z before the send leaves
- **THEN** nothing is sent and the item and the dialog are back on the same draft

### Requirement: Waiting on others
Home SHALL show the conversations Scout is waiting on (`attention.list` with
`state=waiting`) under "Waiting on others": per row, the awaited person's
name, a few words of the conversation's subject and when Scout will check
("Checks Mon"), soonest first, with Nudge now (the nudge dialog) and Not
waiting (with Undo). On desktop it SHALL sit in the aside above Done today;
below 48rem, in a fold after the list, with its count. It SHALL be left out
when Scout waits on nobody, and SHALL refetch with the rest of Home.

#### Scenario: Watching for Ana
- **WHEN** Scout waits on Ana Costa until Monday and on Rui Matos until Friday
- **THEN** "Waiting on others" lists Rui ("Checks Fri") then Ana ("Checks Mon"), and neither counts in "things need you"

#### Scenario: Nudge early
- **WHEN** the user chooses Nudge now on Rui's row
- **THEN** the nudge dialog opens for Rui, and after the send Rui's row says when Scout checks again

### Requirement: Continue with Scout on a need
Each need on Home SHALL offer "Continue with Scout" (the visor before it,
`.` with its hint), after its own actions and in the same button style,
opening the Scout panel for the need's conversation. It SHALL not take one
of the keys 1 to 3. On a server where Scout can't chat it SHALL still be
offered, and the panel SHALL say why.

#### Scenario: Asking about a bill
- **WHEN** Sam selects Marta's Pay need and presses `.`
- **THEN** the Scout panel opens about Marta's conversation, and the need stays selected

### Requirement: Tasks in Do and Pay
Home SHALL include the user's open task items the list returns (those for
today), rendered from `tasks.list` with `ids`, in their bands beside
conversation items and in the list's order. A task row SHALL show its
eyebrow ("Do task" or "Pay task" and its reason, its date among them), where
it came from and its title. Its actions SHALL be Done (Paid for `pay`) `e`, Later
`s` and Not needed `x`, calling `tasks.done`, `tasks.plan` with `later` and
`tasks.not_needed`, each with Undo.

#### Scenario: The lease and the bill
- **WHEN** Ana has a task "Send the signed lease to Ana" due today and an overdue EDP bill task
- **THEN** Now lists the bill ("Pay task · Overdue · 1 day") and Today the lease task ("Do task · Due today"), and pressing `e` on the bill marks it paid with Undo

#### Scenario: Later from Home
- **WHEN** Ana presses `s` on a task and picks Tomorrow
- **THEN** the task leaves Home until tomorrow and Tasks shows it under Later

### Requirement: Home spacing
Home's list SHALL keep a section's room (the `--space-6` token) between
consecutive groups, around the Scout noticed note, and between a group whose
last rows are folding away and what follows. An item opened in place (its
card) SHALL keep the same room above and below it within its group, so it
never touches the note, a neighbouring row or the next group's title. The
Tasks list's groups SHALL keep the same rhythm.

#### Scenario: Reply under Scout noticed
- **WHEN** Home shows the Now group, then Scout noticed, then Today
- **THEN** there is a section's room between Now and the note and between the note and "Today 2"

#### Scenario: An opened item
- **WHEN** Ana opens the last Now item in place, right above Scout noticed
- **THEN** its card keeps a section's room from the note and from the row above it

#### Scenario: Tasks
- **WHEN** Tasks shows Overdue, Today and Coming up
- **THEN** each group keeps the same room from the next

### Requirement: The reason first
Each row of Home SHALL start with an eyebrow line, before the name, in small
semibold capitals in the tertiary text colour: the kind's signal dot and name
(Review, Reply, Decide, Do, Pay, Nudge, Worth a look; "task" after it for a
task), a `·`, then the reason the item is in its band, in the secondary text
colour, or the danger colour for a warning and an overdue item. The reasons
SHALL read, in the user's language: "Possible scam · check first", "Overdue ·
N days" (one day singular), "Due today", "Due tomorrow", "You planned it for
today", "<Name>'s answer was due <day>", "<Name> has waited N days", "Due
<day>" (the short weekday within a week, else the day and month), "<Name> is
waiting for your answer", "Kept here by your rule" and "No date". <Name> SHALL
be the person's first name as Home shows it elsewhere; without a name the
reason SHALL read without one ("Waited N days", "Waiting for your answer",
"An answer was due <day>"). The row's end SHALL then show only the age.

#### Scenario: Pedro waited
- **WHEN** Pedro Alves's reply has waited 4 days
- **THEN** his row's eyebrow reads "Reply · Pedro has waited 4 days" before his name

#### Scenario: An overdue task
- **WHEN** a Pay task is 2 days overdue
- **THEN** its eyebrow reads "Pay task · Overdue · 2 days", the reason in the danger colour

### Requirement: Why it's here
The opened item SHALL show, with the other details, a small heading "Why
it's here" with the visor, then one sentence "In <band> because <reason>."
and the band's description: Now "Scam warnings and anything overdue.", Today
"Due today or tomorrow, planned for today, nudges that are due, and people
who have waited 3 days or more.", This week "Dated in the next 7 days, and
people waiting for your answer.", Whenever "No date, from services, or kept
here by your rules.", in the user's language. Opening it SHALL change
nothing.

#### Scenario: Opened in Today
- **WHEN** Sam opens Pedro's reply, which has waited 4 days
- **THEN** it says "In Today because Pedro has waited 4 days." and the Today description

### Requirement: More in Tasks
When `attention.list` says some open tasks are not for today
(`tasks_elsewhere` above 0), the end of Home's list SHALL say "N more in
Tasks" (one singular) with an "Open Tasks" link showing the Tasks key (`G`
`T`), in the user's language.

#### Scenario: Four more
- **WHEN** Ana has four open tasks that are not for today
- **THEN** the list ends with "4 more in Tasks" and Open Tasks opens Tasks
