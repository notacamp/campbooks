## MODIFIED Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups by priority band, in the order
Now, Today, This week, Whenever, leaving out empty groups, each item in the
list's order. Each group SHALL be headed by its name and its count, Now in
the danger tone. The hero SHALL say how many things need the user, with the
Visor `alert` when a warning is on the list. The selected item SHALL be shown
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
- **THEN** the one due Wednesday is first, and its eyebrow says "Due Wed 14"

#### Scenario: How long it has waited
- **WHEN** Marta's question arrived at 10:00 today, Pedro's on Monday and the bill on 2 October, and it is Friday 15:00
- **THEN** Marta's row says "5 hours ago", Pedro's "4 days ago" and the bill's "2 Oct", each with its full date on hover, and the bill's due date is in its eyebrow

#### Scenario: A nudge after the bills
- **WHEN** the user has a bill overdue by a day, a nudge and a reply from a service with no date
- **THEN** Home says "3 things need you" and lists the bill in Now, the nudge in Today, then the reply in Whenever

#### Scenario: Down the bands with the keys
- **WHEN** the user presses ↓ on the last item of Now
- **THEN** the first item of Today is selected

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

### Requirement: Nudge items
A `nudge` item SHALL say who has not written yet, by the awaited person's
name ("Ana Costa hasn't written yet") when someone else on the email was
awaited, or "No word from Rui Matos yet" when the sender was, with the
sender's sentence quoted on one line, its eyebrow saying when an answer was
due ("Ana's answer was due Mon 12") and, at the row's end, since when it has
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

### Requirement: Tasks in Do and Pay
Home SHALL include the user's open task items the list returns (those for
today), rendered from `tasks.list` with `ids`, in their bands beside
conversation items and in the list's order. A task row SHALL show its
eyebrow ("Do task" or "Pay task" and its reason), its title, where it came
from and its due chip. Its actions SHALL be Done (Paid for `pay`) `e`, Later
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

## ADDED Requirements

### Requirement: The reason first
Each row of Home SHALL start with an eyebrow line, before the name, in small
semibold capitals in the tertiary text colour: the kind's signal dot and name
(Review, Reply, Decide, Do, Pay, Nudge, Worth a look; "task" after it for a
task), a `·`, then the reason the item is in its band, in the secondary text
colour, or the danger colour for a warning and an overdue item. The reasons
SHALL read, in the user's language: "Possible scam · check first", "Overdue ·
N days" (one day singular), "Due today", "Due tomorrow", "You planned it for
today", "<Name>'s answer was due <day>", "<Name> has waited N days", "Due
<day>" (short weekday and day within a week, else day and month), "<Name> is
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
