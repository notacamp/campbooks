## MODIFIED Requirements

### Requirement: Scout noticed
Right after the first group of Home (and where the groups would be when
nothing needs the member), Home SHALL show at most one suggestion from
`conversations.list_rule_notices`: a title line with "Scout noticed" and "n
of N", then what Scout noticed in a sentence built from the counts in the
member's language ("Design Notes sent you 88 emails in the last 60 days, and
you archived or deleted 88 of them unread. Archive them as they arrive?"), set
in the body font for reading; for a suggestion about one kind of a sender's
mail (it has a `pattern`) the sentence SHALL name the kind by its example
subject, as plain text in the locale's quotation marks and cut to about 60
characters at a word ("Northwind sent you 40 emails like “Deals of the week
40” in the last 60 days, …"), with the kind's counts. Then a link "See the N
emails" that opens the search results for the sender's mail in the window
Scout counted (`from:<sender> newer_than:<days>d`, followed by the pattern's
`search` when it has one, such as `subject:"deals of the week"`), then one row
of buttons in one style: the suggestion's own verb ("Archive them…", or "File
under Updates…"), "Something else…" (with the visor before it) and "Not now",
and a More menu holding "Don't suggest this" and "Ask Scout" (where a chat is
mounted). "Not now" and "Don't suggest this" SHALL each show a toast with
Undo, and the next suggestion shows. Both of the first two SHALL open the
smart-rule confirmation, which asks what should happen to the sender's mail
first and says the rule's conditions in words, the template or the words of
a kind included. The button row SHALL never wrap: where it does not fit (a
phone), its buttons fold into More with the rest, each with its name and key.
In a shared inbox it SHALL say the rule would act for everyone in that inbox.
It SHALL not show while Home loads or the list fails or is empty, and at
375px as on desktop it sits after the first group.

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

#### Scenario: One kind of a shop's mail
- **WHEN** Scout noticed a shop's weekly deals by their template, with the example "Deals of the week 40: up to half off garden tools"
- **THEN** the sentence reads "Northwind (orders@shop.example) sent you 40 emails like “Deals of the week 40: up to half off garden tools” in the last 60 days, …", and "See the 40 emails" searches `from:orders@shop.example newer_than:60d subject:"deals of the week"`

#### Scenario: A long example
- **WHEN** the example subject is longer than 60 characters
- **THEN** the sentence quotes it cut at a word, ending with "…"

#### Scenario: One row
- **WHEN** the suggestion shows on a desktop
- **THEN** the verb, Something else… and Not now sit on one row with More after them, and Don't suggest this and Ask Scout are in More with their keys

#### Scenario: One row on a phone
- **WHEN** the suggestion shows at 375px
- **THEN** the row holds the verb and More, and More lists Something else…, Not now, Don't suggest this and Ask Scout
