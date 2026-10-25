## MODIFIED Requirements

### Requirement: Scout noticed
The module SHALL keep, per inbox, smart rules Scout offers for mail the inbox
gets a lot of (`conversations.rule_notices`), worked out by
`conversations.notice_rules` (one job per inbox, at most every
`CAMPBOOKS_CONVERSATIONS__NOTICES__EVERY_HOURS`, default 24, started by
`conversations.list_rule_notices` for an inbox not looked at since) from
facts and counts, never a body: per sender over the window
(`WINDOW_DAYS`, 60), how many conversations (at least `MIN_MESSAGES`, 8) on
how many days (at least `MIN_DAYS`, 4) and how regularly, the stream most are
sorted to, and what the inbox did with them. A sender SHALL never be noticed
when it is a person (not list, automatic or no-reply mail), the inbox writes
to it, or one of its messages looked suspicious. A sender's mail SHALL be
split into kinds by equal subject template, a template with at least
`MIN_KIND_MESSAGES` (4) conversations on at least `MIN_KIND_DAYS` (2) days
being a kind; each kind SHALL be counted on its own, and a message of it that
asks for a payment owed, needed a reply or action the member did not
dismiss, or was replied to SHALL block that kind only. When every kind (and
the mail left over, when it holds at least `MIN_KIND_MESSAGES`) would be
offered what all the sender's mail would, and none of it was replied to,
asked anything or asks for a payment, there SHALL be one suggestion for the
sender, as before; otherwise one per kind that qualifies, its rule the sender
and that `subject_template`, and never one for the whole sender. For such a
sender whose mail left over is at least 30% of it and at least
`MIN_KIND_MESSAGES`, the module SHALL ask the `complete` model, only while it
may receive mail on this server, for at most `MODEL_SENDERS_PER_PASS` (3, 0
never) senders per inbox and pass, the most frequent first, with that
sender's most recent distinct subjects only (`MODEL_SUBJECTS`, 40, never a
body, a name or an address) under a fixed instruction that they are data, for
kinds of email each as a run of words (a strict JSON schema, at most 6, each
normalised to 1 to 8 words), counted against the workspace's daily AI limit
as background work and kept per sender for `REGROUP_DAYS` (7), none found
included, with no transaction held while it answers. Each phrase SHALL be a
kind of the sender's mail whose subject holds it, as the rule engine's
`subject_contains` matches it, when that mail qualifies as a kind and is not
half in a kind already offered, its rule the sender and that
`subject_contains`. With the model off, refused, failing or over the limit,
nothing SHALL be asked or kept and the template kinds SHALL be offered as
they would be. A sender whose mail is mixed with no kind that qualifies SHALL
get nothing. Mail archived or deleted unread at least
`ARCHIVED_UNREAD_PERCENT_MIN` (70%) of the time SHALL be offered `archive`
and `surface: never` ("archive it as it arrives"); else mail opened at most
`OPENED_PERCENT_MAX` (25%) of the time SHALL be offered `move_to_stream` (the
stream most are in, Updates for Primary) and `surface: never` ("keep it off
Home"). At most `MAX_PER_INBOX` (5) per inbox, one per sender, kind and
pattern, none for a sender a rule of the inbox already names with no subject
condition, none for a kind a rule names with that sender and that template or
those words, and none whose rule would change nothing; each pass refreshes
them and drops those that no longer hold. `GET …/rule-notices`
(`conversations.list_rule_notices`, `conversations:read`) SHALL list the
open ones of the inboxes the caller may write in, the most mail first, each
with its sender, its rule, its counts (received, opened, archived unread, last
week, the window, the cadence, those of its kind), whether the inbox is
shared and, for a kind, its `pattern`: `source` (`template` or `phrase`), one
recent subject of it as `example`, and the `search` fragment that narrows a
search to it (`subject:"<words>"`). `POST …/rule-notices/{id}/answer`
(`conversations.answer_rule_notice`, `conversations:write`, `not_now` or
`never`, undo `conversations.clear_rule_notice_answer`, `DELETE` on the same
path) SHALL hide it from that member for `NOT_NOW_DAYS` (14), or for good for
that sender, kind and pattern; other members still see it.
`conversations.create_rule` with the suggestion's `notice_id` SHALL create
its rule active with origin `suggestion` (the member confirmed it) and accept
the suggestion, whatever actions the member chose: Scout's guess, or another
goal they picked ("Highlight them", filing under Newsletters, marking read, or
what Scout made of their own words), since the member's yes is to what they
were shown; a suggestion of another inbox, or not open, answers 400
`invalid_rule`. Removing that rule with `restore` within 15 minutes SHALL
offer the suggestion again, and bringing the rule back
(`conversations.restore_rule`) SHALL accept it again, so it is neither offered
twice nor saved as a second rule. A pass over an inbox with no inbound mail in
the window yet (its first sync still to come) SHALL count for 10 minutes
only, so a list soon after the first sync looks again, and a dormant inbox at
most that often. A caller who cannot see the inbox SHALL get 404, a reader or
an app 403. Removing an inbox's connection with its mail kept SHALL forget its
suggestions and the model's groupings; a departing member's answers SHALL go.

#### Scenario: Build notifications
- **WHEN** Sam got 34 notifications from builds@ci.example.com over 34 days and opened 2
- **THEN** Home's list offers "keep them off Home and file them under Updates" with 34 received and 2 opened, daily

#### Scenario: A shop's promotions and its shipping notices
- **WHEN** a shop sent Sam 12 weekly deals he never opened and 6 shipping notices he read
- **THEN** the list offers to keep the deals off Home (the shop and the template "deals of the week #", 12 received, the newest deal as the example) and nothing for the shipping notices or the whole shop

#### Scenario: A bill in one kind
- **WHEN** one of a shop's invoices asks for a payment owed and its deals are never opened
- **THEN** only the deals are offered

#### Scenario: Mixed mail and no kind
- **WHEN** a sender's deals are opened half the time and one of its shipping notices was answered
- **THEN** nothing is offered for that sender

#### Scenario: The model's kinds
- **WHEN** a shop's promotions each have a subject of their own, are never opened, and the model proposes "sale starts now", "has shipped" and "boots"
- **THEN** the list offers the shop and the words "sale starts now" with 10 received, the read shipping notices and the one boots email get nothing, the model saw the subjects and nothing else, and a pass the next day does not ask it again

#### Scenario: The model off
- **WHEN** the `complete` role is off the instance without the opt-in and the shop sends deals, promotions and shipping notices
- **THEN** the model is not asked, nothing is kept, and the deals are offered by their template

#### Scenario: Not now
- **WHEN** Sam answers Not now and Ana, who shares the inbox, lists suggestions
- **THEN** Sam does not see it for two weeks and Ana still does

#### Scenario: One kind answered
- **WHEN** Sam answers "Don't suggest this" to the deals and the shop's digests are offered too
- **THEN** Sam never sees the deals again and still sees the digests

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

### Requirement: Rule suggestions
`GET …/conversations/{conversation_id}/rule-suggestions?action=…` (the
action and a stream or label id only)
(`conversations.suggest_rules`, `conversations:read`, `sender` role) SHALL,
after a member's move, archive, label, mark read or Not needed on one
conversation, answer up to five likenesses from broad to narrow (the
sender, the List-Id, the sender with words the `complete` model proposed for
its subjects during Scout noticed's pass that this subject holds (kept, never
asked for here), the sender or domain with the subject words or template),
each with its conditions, its preview and whether it is the default, or no
suggestion. The default SHALL be, for a machine sender the inbox never wrote
to, the sender when its mail over Scout noticed's window is all one kind (as
Scout noticed splits it), else the model's words, else the template, else
none; for other senders the List-Id, else the template. For a correspondent
there SHALL be no default, and archive, mark read and Not needed SHALL get no
suggestion. There SHALL be a suggestion only when the default covers at least
two other conversations or the message has a List-Id (with no default, when
any likeness does), never for a suspicious
conversation, and never while the member dismissed it. After the member
reported the conversation as spam (`action=report`) it SHALL offer to report
mail like it as it arrives, suspicious as it is: when the receiving provider
says its sender failed DMARC under a `reject` or `quarantine` policy (so the
domain's genuine mail passes), the sender's domain with `sender_checks_fail` first, then the
sender; else the sender only. A whole domain SHALL never be offered to the
trash or spam (people the inbox writes to may send from it). After Already paid on its Pay
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

#### Scenario: A shop's mixed mail
- **WHEN** Sam archives one of a shop's weekly deals, which he never opens, while he reads its shipping notices
- **THEN** the suggestions are the shop and the shop with the deals' template, and the default is the template

#### Scenario: The model's words kept
- **WHEN** Scout noticed's pass kept "sale starts now" for that shop and Sam archives one of its promotions
- **THEN** the shop with the words "sale starts now" is offered and is the default, and no model is asked

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
