## MODIFIED Requirements

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
- **WHEN** Home has Review, Reply and Pay groups and Scout noticed something
- **THEN** the note sits between the Review group and the Reply group

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

## ADDED Requirements

### Requirement: Home spacing
Home's list SHALL keep a section's room (the `--space-6` token) between
consecutive groups, around the Scout noticed note, and between a group whose
last rows are folding away and what follows. An item opened in place (its
card) SHALL keep the same room above and below it within its group, so it
never touches the note, a neighbouring row or the next group's title. The
Tasks list's groups SHALL keep the same rhythm.

#### Scenario: Reply under Scout noticed
- **WHEN** Home shows the Review group, then Scout noticed, then Reply
- **THEN** there is a section's room between Review and the note and between the note and "Reply 2"

#### Scenario: An opened item
- **WHEN** Ana opens the last Review item in place, right above Scout noticed
- **THEN** its card keeps a section's room from the note and from the row above it

#### Scenario: Tasks
- **WHEN** Tasks shows Overdue, Today and Coming up
- **THEN** each group keeps the same room from the next
