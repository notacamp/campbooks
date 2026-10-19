## MODIFIED Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups, in the order Review, Reply,
Decide, Do, Pay, Nudge, leaving out empty groups, each item in the list's order. The
hero SHALL say how many things need the user, with the Visor `alert` when
Review has an item. The selected item SHALL be shown by the selected background and a full focus
ring, never by a coloured stripe on one edge. Each item SHALL show who it is from, its sentence (from a
template chosen by reason and by the rule that decided it, in the user's
language), the sender's sentence quoted on one line, and, at the row's end,
how long it has waited above its due date or overdue. The age SHALL count
from when the item's cause arrived (`since`), in the user's language: minutes
or hours the same day, "Yesterday" and days ago in the week after, the date
from a week on (with the year when it is another year's), with the full date
and time on hover; every row SHALL show it, dated or not. Rows SHALL come from
the owning module's batch read (`conversations.list` with `ids`), not from
`attention`.

#### Scenario: A mixed morning
- **WHEN** the user has a phishing warning, two replies, a dated ask and a bill
- **THEN** Home says "5 things need you", Review comes first with the warning, then Reply, Do and Pay, and nothing shows a percentage

#### Scenario: Due soonest first
- **WHEN** the Reply group holds a question due Wednesday and an older one with no date
- **THEN** the one due Wednesday is first

#### Scenario: How long it has waited
- **WHEN** Marta's question arrived at 10:00 today, Pedro's on Monday and the bill on 2 October, and it is Friday 15:00
- **THEN** Marta's row says "5 hours ago", Pedro's "4 days ago" and the bill's "2 Oct" above "Due 30 Oct", each with its full date on hover

#### Scenario: A nudge at the end
- **WHEN** the user has a reply and a nudge
- **THEN** Home says "2 things need you", Reply comes first and Nudge last

## ADDED Requirements

### Requirement: Nudge items
A `nudge` item SHALL say who has not written yet, by the awaited person's
name ("Ana Costa hasn't written yet") when someone else on the email was
awaited, or "No word from Rui Matos yet" when the sender was, with the
sender's sentence quoted on one line and, at the row's end, since when it
has waited ("Asked 14 Oct") and, after a nudge, how many were sent
("Nudged once"). Its actions SHALL be Nudge (1, the nudge dialog), Heard
from them (E, `conversations.resolve_need`), Later (S) and Not waiting (X,
the need cleared as not needed, a correction when its trust is `check`),
each with the row's confirmation and Undo (Z). An item in an inbox the user
may only read SHALL offer only Later.

#### Scenario: Ana hasn't written
- **WHEN** the wait on Ana Costa in João's conversation turns into a nudge on Monday
- **THEN** Home lists, under Nudge, "Ana Costa hasn't written yet", João's line quoted, "Asked 14 Oct", with Nudge on 1

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
- **THEN** the nudge dialog opens for Rui, and after the send his row says when Scout checks again
