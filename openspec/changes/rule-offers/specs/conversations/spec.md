## MODIFIED Requirements

### Requirement: Taught rules
The module SHALL store taught rules: deterministic condition → action pairs
for one inbox, with one to four conditions that must all hold and one to four
actions, at most one per slot (`stream`, `archive`, `read`, `needs_reply`,
`asks_action`, `surface`, `bin` (the trash or spam), `money`; `add_label` is
additive). Conditions SHALL be
`sender_address is`, `sender_domain is` (never a free-mail domain on its
own), `list_id is`, `subject_template is`, `subject_contains` (whole words,
case- and accent-insensitive), `header is|present` (a stored header),
`label has`, `stream is` and `sender_checks_fail` (the receiving provider's
authentication results say the sender's domain failed its checks: a DMARC
fail, or neither a DMARC pass for the From domain nor an aligned DKIM pass;
absent results never hold it; never on its own). Actions SHALL be
`move_to_stream`, `archive`,
`add_label`, `mark_read`, `needs_reply` true or false, `asks_action` true or
false, `surface` always or never, `trash`, `spam` and `mark_paid`. A rule
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

#### Scenario: Failed checks alone
- **WHEN** a rule's only condition is `sender_checks_fail`
- **THEN** the response is 400 with code `invalid_rule`

#### Scenario: A stored rule naming a public suffix
- **WHEN** a rule stored with `sender_domain is github.io` meets mail from `a@a.github.io`
- **THEN** it does not match, while a rule for `github.com` still matches `a@mail.github.com`

### Requirement: Suspicious mail is never hidden by a rule
When a message has a true `suspicious` answer from any source, or its
conversation has an open Review need of any kind (a warning or a question to
the member), rules SHALL NOT archive it, mark it
read, move it out of `primary`, answer a need question false, apply
`surface: never` or say its bills are paid; the Review need SHALL stay. A
rule that puts mail in the trash or reports it as spam SHALL act on such mail
(it is about it), unless a rule saying `surface: always` matches it. Such a
conversation SHALL start no rule suggestion other than the one after it was
reported.

#### Scenario: A look-alike sender
- **WHEN** a phishing message claims to be from notifications@github.com and the rules answer `suspicious` true
- **THEN** the archive and move rules for that sender leave it in Primary with its Review need

#### Scenario: Never see emails like this again
- **WHEN** a rule reports as spam mail from `paypal.com` that fails its sender checks, and a suspicious message from service@paypal.com arrives failing DMARC
- **THEN** it goes to spam, at the provider too, and real PayPal mail passing its checks stays

### Requirement: Rule suggestions
`GET …/conversations/{conversation_id}/rule-suggestions?action=…` (the
action and a stream or label id only)
(`conversations.suggest_rules`, `conversations:read`, `sender` role) SHALL,
after a member's move, archive, label, mark read or Not needed on one
conversation, answer up to four likenesses from broad to narrow (the
sender, the List-Id, the sender or domain with the subject words or template),
each with its conditions, its preview and whether it is the default, or no
suggestion. The default SHALL be the sender for a machine sender the inbox
never wrote to, else the List-Id, else the template. For a correspondent
there SHALL be no default, and archive, mark read and Not needed SHALL get no
suggestion. There SHALL be a suggestion only when the default covers at least
two other conversations or the message has a List-Id, never for a suspicious
conversation, and never while the member dismissed it. After the member
reported the conversation as spam (`action=report`) it SHALL offer to report
mail like it as it arrives, suspicious as it is: when the receiving provider
says its sender failed its checks, the sender's domain with
`sender_checks_fail` first (never the bare domain), then the sender; else the
sender, then its domain unless a free-mail one. After Already paid on its Pay
need (`action=paid`) it SHALL offer `mark_paid` for the sender, then its
domain unless a free-mail one, a correspondent included. These two SHALL be
offered with nothing like it in the inbox yet (they are about mail still to
come), the first the default, and SHALL answer no suggestion
(`not_teachable`) when the conversation was not reported, or its need not
answered Already paid. A correspondent SHALL get no report offer.
`POST …/conversations/{conversation_id}/rule-suggestions/dismiss`
(`conversations.dismiss_rule_suggestion`, with the suggestion's `key`) SHALL
hide it from that member for 30 days, and for good after a second dismissal.
With no default, a dismissal of any likeness offered SHALL hide the
suggestion.

#### Scenario: The 1,000 notifications
- **WHEN** Ana moves one GitHub notification to Updates and 1,000 more from notifications@github.com are in Primary
- **THEN** the suggestion's default is "sender is notifications@github.com" with 1,000, followed by the acme/web List-Id with 612 and the sender with the subject's template

#### Scenario: A person
- **WHEN** Ana archives a message from Marta, whom her inbox has written to
- **THEN** there is no suggestion

#### Scenario: Just this one
- **WHEN** Ana dismisses the suggestion and moves another GitHub notification the next day
- **THEN** there is no suggestion

#### Scenario: A fake PayPal reported
- **WHEN** Ana reports a message from service@paypal.com that failed DMARC, and another fake sits in her inbox next to real PayPal mail
- **THEN** the offer's default is "paypal.com, failing its sender checks → spam", it would change 1, and the real mail is not counted

#### Scenario: Already paid
- **WHEN** Ana says an EDP bill is already paid and asks for suggestions with `action=paid`
- **THEN** the default is "from faturas@edp.example → its bills are paid"

## ADDED Requirements

### Requirement: Trash and spam rules
A rule's `trash` or `spam` action SHALL put a new matching message in the
trash or report it as spam as it arrives, and every matching conversation
when applied to existing mail, as the trash and spam operations do (the
provider is told; trash and spam exclude each other), recording the
application so removing the rule with `restore` takes each one back out
where it was. A member who takes a conversation out of the trash or spam
SHALL make it an exception of the rule that put it there.

#### Scenario: Undo all
- **WHEN** a rule reporting fake PayPal mail as spam is applied to two conversations and removed with `restore`
- **THEN** both come back out of spam to where they were

### Requirement: Bills a rule says are paid
A rule's `mark_paid` action SHALL answer the money question `paid` (source
`taught`, acted) for the mail it matches, so no "Is it paid?" Pay need is
raised for it and an open one is cleared with cause `rule`; a reply or an
action the same mail asks for SHALL still be raised.

#### Scenario: EDP from now on
- **WHEN** a rule says faturas@edp.example's bills are paid and a new EDP invoice arrives that the model reads as owed
- **THEN** no Pay need is raised for it

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
`suggestion` (the member confirmed it) and accept the suggestion; a
suggestion of another inbox, or not open, answers 400 `invalid_rule`.
Removing that rule with `restore` within 15 minutes SHALL offer the
suggestion again. A caller who cannot see the inbox SHALL get 404, a reader
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
- **THEN** the rule acted at once, its mail is put back, and the suggestion is offered again

#### Scenario: Someone else's inbox
- **WHEN** Leo, who is not in Sam's inbox, lists suggestions or answers one by its id
- **THEN** he sees none, and the answer is 404
