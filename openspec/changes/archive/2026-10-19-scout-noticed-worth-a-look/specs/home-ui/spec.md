## ADDED Requirements

### Requirement: Worth a look
Home SHALL show mail a rule the member taught with "Highlight them" in a last
group, "Worth a look", after Pay, with a signal colour of its own (the
`signal-look` token): one item per conversation, saying "You asked to see mail
from <name>". Its row SHALL offer Seen (Done, E) and Snooze (S) only: no
Reply, and no Not needed, since nothing is asked of the member. Seen SHALL
fold the row with "Seen · <name>" and Undo like Done. The item SHALL leave
Home by itself once the message is opened, and Done today SHALL not list that.

#### Scenario: Seen
- **WHEN** Ana presses E on a Worth a look item
- **THEN** it folds into "Seen · Design Notes" with Undo, and the conversation is not archived

#### Scenario: Opened elsewhere
- **WHEN** Ana opens the highlighted conversation from the inbox
- **THEN** its item leaves Home, and Done today does not list it

## MODIFIED Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups, in the order Review, Reply,
Decide, Do, Pay, Worth a look, leaving out empty groups, each item in the list's order. The
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

### Requirement: Scout noticed
Right after the first group of Home (and where the groups would be when
nothing needs the member), Home SHALL show at most one suggestion from
`conversations.list_rule_notices`: "Scout noticed", "n of N", what Scout
noticed in a sentence built from the counts in the member's language ("Design
Notes sent you 88 emails in the last 60 days, and you archived or deleted 88
of them unread. Archive them as they arrive?"), and four buttons in one style:
the suggestion's own verb ("Archive them…", or "File under Updates…"),
"Something else…" (with the visor before it), "Not now" and "Don't suggest
this" (each of the last two with a toast and Undo, and the next suggestion
shows). Both of the first two SHALL open the smart-rule confirmation, which
asks what should happen to the sender's mail first. In a shared inbox it
SHALL say the rule would act for everyone in that inbox. It SHALL not show
while Home loads or the list fails or is empty, and at 375px as on desktop it
sits after the first group.

#### Scenario: Two suggestions
- **WHEN** Scout noticed build notifications and job alerts
- **THEN** Home shows "1 of 2" with the build notifications; Not now shows "1 of 1" with the job alerts, and Undo brings the first back

#### Scenario: Higher on Home
- **WHEN** Home has Review, Reply and Pay groups and Scout noticed something
- **THEN** the note sits between the Review group and the Reply group

#### Scenario: Nothing needs the member
- **WHEN** nothing needs the member and Scout noticed something
- **THEN** the note shows under the hero, where the groups would be
