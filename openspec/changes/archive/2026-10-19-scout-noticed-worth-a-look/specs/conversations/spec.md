## ADDED Requirements

### Requirement: Worth a look
A taught rule with `worth_a_look` SHALL answer a question `worth_a_look` (`true`,
source `taught`) on each new inbound message that matches it, and on the
mail already there when applied to it (unread mail only counts as changed). A
conversation whose newest inbound message carries that answer and raises no
other need (a reply, a task, a bill or a warning comes first: one need per
conversation) SHALL have a need with reason and question `worth_a_look`,
`trust` `sure`, basis `taught`, never urgent and never a question. It SHALL
not pin, archive, mark read or move the mail: the conversation stays where
it was and can be archived like any other. The need SHALL clear itself as
`opened` once that message is read, here or at the provider, so marking a
conversation read or unread (`conversations.mark_read`, `mark_unread`) SHALL
work its need out again; a member's Seen is Done, with no correction filed.
`surface` never wins over `worth_a_look` (nothing raised).

#### Scenario: Highlighting a digest
- **WHEN** Sam's active rule `worth_a_look` matches 12 unread digests from one sender
- **THEN** each of the 12 conversations has an open `worth_a_look` need and none is archived or read

#### Scenario: Opened once
- **WHEN** Sam opens one of them
- **THEN** its need is cleared with cause `opened` and the other 11 stay open

#### Scenario: A reply comes first
- **WHEN** a highlighted sender's message also needs a reply
- **THEN** the conversation's need is Reply, not Worth a look

## MODIFIED Requirements

### Requirement: Clearing a need
A need SHALL be cleared, with its cause, when: a message is sent from the
inbox in the conversation after the message that raised it (`replied`; clears
`reply` and `do`), the conversation is archived (`archived`; clears `reply` and
`review`), every message is in the trash or spam or removed (`trashed`; clears
any), the member opens the message of a "Worth a look" need (`opened`; clears
`worth_a_look` only: the message is read, here or at the provider), or a
member resolves it. A new inbound message SHALL derive the need
again from that message (`superseded` for the old one). When the open need's
own message, read again, raises no need and no member confirmed it, the need
SHALL be cleared as `superseded`. Changes SHALL come from the provider or
from members alike.

#### Scenario: Replied from the phone
- **WHEN** Ana replies to Marta from Gmail on her phone and the sent message reaches the conversation
- **THEN** the `reply` need is cleared with cause `replied`

#### Scenario: Archiving a bill
- **WHEN** a member archives a conversation with a `pay` need
- **THEN** the need stays open until a member chooses Done or Not needed

#### Scenario: Opening a highlighted message
- **WHEN** Ana opens the conversation of a `worth_a_look` need, or reads it on her phone and the read state reaches the conversation
- **THEN** the need is cleared with cause `opened`, and a `pay` need on an opened conversation stays open

#### Scenario: Done in a shared inbox
- **WHEN** Ana chooses Done on a need in an inbox she shares with Leo
- **THEN** the need is cleared for both of them

#### Scenario: A question raised against a settled answer
- **WHEN** a conversation has an open `check` need whose message's accepted answers say it asks nothing, and the conversation's need is refreshed
- **THEN** the need is cleared with cause `superseded`, announced once, and not shown under Done today

### Requirement: Taught rules
The module SHALL store taught rules: deterministic condition → action pairs
for one inbox, with one to four conditions that must all hold and one to four
actions, at most one per slot (`stream`, `archive`, `read`, `needs_reply`,
`asks_action`, `surface`, `bin` (the trash or spam), `money`, `highlight`;
`add_label` is additive). Conditions SHALL be
`sender_address is`, `sender_domain is` (never a free-mail domain on its
own), `list_id is`, `subject_template is`, `subject_contains` (whole words,
case- and accent-insensitive), `header is|present` (a stored header),
`label has`, `stream is` and `sender_checks_fail` (the receiving provider's
authentication results hold `dmarc=fail` under a published `reject` or
`quarantine` policy the receiver applied (not `dis=none`, its sign of a
trusted forwarder or list), so the domain's genuine mail passes; a policy that
cannot be read, and absent results, never hold it; never on its own; never
for mail from an address the inbox writes to, on arrival and over existing
mail alike). Actions SHALL be
`move_to_stream`, `archive`,
`add_label`, `mark_read`, `needs_reply` true or false, `asks_action` true or
false, `surface` always or never, `trash`, `spam`, `mark_paid` and
`worth_a_look` ("Highlight them": see Worth a look). A rule with
`worth_a_look` SHALL hide nothing: with `archive`, `mark_read`, `trash`,
`spam`, `mark_paid`, `surface` never, a `false` `needs_reply` or `asks_action`
or a move out of Primary it answers 400 `invalid_rule`. A rule
SHALL record its origin
(`action`, `onboarding`, `instruction`, `correction`, `suggestion`), the
member's words for an instruction, its author, and its state (`proposed`,
`active`, `paused`, `removed`). Taught rules are member data: the built-in
email signals stay the module's only rules of its own. A `sender_domain`
condition SHALL match only while its value is an owned domain (two labels or
more, not a public suffix); a stored rule naming one that is now a public
suffix (`github.io`) matches nothing and reads as paused with
`paused_reason` `domain_too_broad`, still removable by a member. Confirming a
proposal SHALL validate it as creating one does, so a proposal whose domain is
now a public suffix answers 400 `invalid_rule` and is not kept.

#### Scenario: A rule from a move
- **WHEN** Ana creates a rule with `sender_address is notifications@github.com` and `move_to_stream updates`
- **THEN** it is stored active for her inbox with origin `action` and Ana as its author

#### Scenario: A free-mail domain alone
- **WHEN** a rule's only condition is `sender_domain is gmail.com`
- **THEN** the response is 400 with code `invalid_rule` and nothing is stored

#### Scenario: Two actions in one slot
- **WHEN** a rule asks to move to `updates` and to `receipts`, or to put mail in the trash and in spam
- **THEN** the response is 400 with code `invalid_rule`

#### Scenario: Highlighting and hiding
- **WHEN** a rule asks for `worth_a_look` and `archive`
- **THEN** the response is 400 with code `invalid_rule` and nothing is stored

#### Scenario: Failed checks alone
- **WHEN** a rule's only condition is `sender_checks_fail`
- **THEN** the response is 400 with code `invalid_rule`

#### Scenario: A whole domain to the bin
- **WHEN** a rule would put all mail from `sender_domain is shop.example` in the trash or spam, with nothing narrower than the domain and no `sender_checks_fail`
- **THEN** the response is 400 with code `invalid_rule`

#### Scenario: Pretending, but someone the inbox writes to
- **WHEN** a rule reports mail from paypal.com that fails its sender checks, and mail failing DMARC under p=REJECT arrives from billing@paypal.com, whom Sam writes to, and from service@paypal.com, whom he does not
- **THEN** only the mail from service@paypal.com is reported, and mail failing DMARC under p=NONE is never

#### Scenario: A stored rule naming a public suffix
- **WHEN** a rule stored with `sender_domain is github.io` meets mail from `a@a.github.io`
- **THEN** it does not match, while a rule for `github.com` still matches `a@mail.github.com`

### Requirement: Scout noticed
The module SHALL keep, per inbox, smart rules Scout offers for mail the inbox
gets a lot of (`conversations.rule_notices`), worked out by
`conversations.notice_rules` (one job per inbox, at most every
`CAMPBOOKS_CONVERSATIONS__NOTICES__EVERY_HOURS`, default 24, started by
`conversations.list_rule_notices` for an inbox not looked at since) from
facts and counts only, never the text: per sender over the window
(`WINDOW_DAYS`, 60), how many conversations (at least `MIN_MESSAGES`, 8) on
how many days (at least `MIN_DAYS`, 4) and how regularly, the stream most are
sorted to, and what the inbox did with them. A sender SHALL never be noticed
when it is a person (not list, automatic or no-reply mail), the inbox writes
to it, one of its messages looked suspicious, asks for a payment owed, needed
a reply or action the member did not dismiss, or was replied to. Mail
archived or deleted unread at least `ARCHIVED_UNREAD_PERCENT_MIN` (70%) of
the time SHALL be offered `archive` and `surface: never` ("archive it as it
arrives"); else mail opened at most `OPENED_PERCENT_MAX` (25%) of the time
SHALL be offered `move_to_stream` (the stream most are in, Updates for
Primary) and `surface: never` ("keep it off Home"). At most `MAX_PER_INBOX`
(5) per inbox, one per sender and kind, none for a sender a rule of the inbox
already names or whose rule would change nothing; each pass refreshes them and
drops those that no longer hold. `GET …/rule-notices`
(`conversations.list_rule_notices`, `conversations:read`) SHALL list the
open ones of the inboxes the caller may write in, the most frequent sender
first, each with its sender, its rule, its counts (received, opened, archived
unread, last week, the window, the cadence) and whether the inbox is shared.
`POST …/rule-notices/{id}/answer` (`conversations.answer_rule_notice`,
`conversations:write`, `not_now` or `never`, undo
`conversations.clear_rule_notice_answer`, `DELETE` on the same path) SHALL
hide it from that member for `NOT_NOW_DAYS` (14), or for good for that sender
and kind; other members still see it. `conversations.create_rule` with the
suggestion's `notice_id` SHALL create its rule active with origin
`suggestion` (the member confirmed it) and accept the suggestion, whatever
actions the member chose: Scout's guess, or another goal they picked ("Highlight
them", filing under Newsletters, marking read, or what Scout made of their own
words), since the member's yes is to what they were shown; a
suggestion of another inbox, or not open, answers 400 `invalid_rule`.
Removing that rule with `restore` within 15 minutes SHALL offer the
suggestion again, and bringing the rule back (`conversations.restore_rule`)
SHALL accept it again, so it is neither offered twice nor saved as a second
rule. A pass over an inbox with no inbound mail in the window yet (its first
sync still to come) SHALL count for 10 minutes only, so a list soon after
the first sync looks again, and a dormant inbox at most that often. A caller who cannot see the inbox SHALL get 404, a reader
or an app 403. Removing an inbox's connection with its mail kept SHALL forget
its suggestions; a departing member's answers SHALL go.

#### Scenario: Build notifications
- **WHEN** Sam got 34 notifications from builds@ci.example.com over 34 days and opened 2
- **THEN** Home's list offers "keep them off Home and file them under Updates" with 34 received and 2 opened, daily

#### Scenario: Not now
- **WHEN** Sam answers Not now and Ana, who shares the inbox, lists suggestions
- **THEN** Sam does not see it for two weeks and Ana still does

#### Scenario: Saying yes
- **WHEN** Sam saves the suggestion's rule with its `notice_id`, then undoes it
- **THEN** the rule acted at once, its mail is put back, and the suggestion is offered again; undoing that brings the rule back and the suggestion goes, and saving it again answers 400

#### Scenario: Another goal
- **WHEN** Sam answers a suggestion to archive builds@ci.example.com with a rule that only has `worth_a_look`, passing the suggestion's `notice_id`
- **THEN** the rule is created active with origin `suggestion`, the suggestion is accepted and never offered again

#### Scenario: Before the first sync
- **WHEN** Home lists suggestions before any of Sam's mail arrived, and then his first sync lands
- **THEN** the first pass finds nothing and counts for ten minutes only, and a list after that looks again and offers the suggestion

#### Scenario: Someone else's inbox
- **WHEN** Leo, who is not in Sam's inbox, lists suggestions or answers one by its id
- **THEN** he sees none, and the answer is 404
