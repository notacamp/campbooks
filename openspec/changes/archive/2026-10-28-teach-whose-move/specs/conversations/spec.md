## MODIFIED Requirements

### Requirement: Taught rules
The module SHALL store taught rules: deterministic condition → action pairs
for one inbox, with one to four conditions that must all hold and one to four
actions, at most one per slot (`stream`, `archive`, `read`, `needs_reply`,
`asks_action`, `surface`, `bin` (the trash or spam), `money`, `highlight`, `wait`;
`add_label` is additive). Conditions SHALL be
`sender_address is`, `sender_domain is` (never a free-mail domain on its
own), `list_id is`, `subject_template is`, `subject_contains` (whole words,
case- and accent-insensitive), `header is|present` (a stored header),
`label has`, `stream is`, `member_not_in_to` (none of the inbox's addresses is
in the message's `To`: it was only copied, blind copied or sent to a list; read
from the addresses alone, and false when the inbox is not known) and
`sender_checks_fail` (the receiving provider's
authentication results hold `dmarc=fail` under a published `reject` or
`quarantine` policy the receiver applied (not `dis=none`, its sign of a
trusted forwarder or list), so the domain's genuine mail passes; a policy that
cannot be read, and absent results, never hold it; never on its own; never
for mail from an address the inbox writes to, on arrival and over existing
mail alike). Actions SHALL be
`move_to_stream`, `archive`,
`add_label`, `mark_read`, `needs_reply` true or false, `asks_action` true or
false, `surface` always or never, `trash`, `spam`, `mark_paid`,
`worth_a_look` ("Highlight them": see Worth a look) and `wait_on_addressed`
("Wait on them": see Wait on them). A rule with
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

#### Scenario: Waiting without "not in To"
- **WHEN** a rule asks for `wait_on_addressed` without the condition `member_not_in_to`, or with `member_not_in_to` as its only condition
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
answered Already paid. A correspondent SHALL get no report offer. After the member chose to wait on
the person a message was written to (`action=waiting`) it SHALL offer one rule,
`sender_address` of the sender with `member_not_in_to`, doing
`wait_on_addressed`, as the default and only suggestion, with the working days
a wait lasts (`wait_days`), and only when all of these hold: the message went
to the member without them in `To`; the member's own answer on it was
"someone else has the move" on exactly the one person the rule would name; and
the member's active (not withdrawn) answers of that kind on mail from this
sender in this inbox number at least two, this one included, counted from the
`next_move` corrections inside the member's workspace. Otherwise it SHALL answer
no suggestion (`too_few` below two, `not_teachable` for the rest). Undoing the
answer withdraws its corrections, so the offer is gone.
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

#### Scenario: Nudge, twice
- **WHEN** Sam answered "Nudge Ana" on João's mail to Ana, copying his inbox, and answers it again on another of João's
- **THEN** `action=waiting` offers "João's mail, with Sam not in To → wait on the person it was written to" with `wait_days` 3; after the first answer alone it says `too_few`, and after Undo of the second it says `not_teachable`

#### Scenario: Already paid
- **WHEN** Ana says an EDP bill is already paid and asks for suggestions with `action=paid`
- **THEN** the default is "from faturas@edp.example → its bills are paid"

## ADDED Requirements

### Requirement: Wait on them
A rule whose action is `wait_on_addressed` SHALL, on a new message that
matches its conditions (which SHALL include `member_not_in_to`), answer as a
taught source what the member's own choice "Nudge {person}" would have filed:
`next_move` is `someone_else` and `awaited` names the one person in the
message's `To` who may be awaited (the facts' list: not the inbox, not the
sender, no address that cannot answer, no machine sender). It SHALL answer
nothing when `To` holds none, or more than one, such person, so the question is
asked as before. It SHALL read addresses and headers only, never what a message
says. The conversation SHALL then wait on that person at once, `sure`, with the
rule as its basis, its nudge time and every guard of any other wait unchanged
(never the inbox itself, never list or automatic mail, never a sorted stream
other than Primary, heard-back clears it, the member writing to them re-arms
it). The rule's answer SHALL replace the model's for those two questions, so the
model's own "needs a reply" or "asks for an action" raises nothing; a "needs a
reply" or "asks for an action" that another taught rule gives for the same mail
SHALL still raise its Reply or Do, which comes before a wait, so the member's
own rules are never silently overridden. The rule SHALL not act on suspicious
mail, or mail that fails its sender checks. Nothing SHALL be sent on the rule's
behalf: the nudge stays the member's. Removing the rule or an exception SHALL
withdraw its two answers like any rule's.

#### Scenario: A new message
- **WHEN** the rule exists for João and he writes to Ana Costa, copying the inbox, and the model reads it as asking the member for a reply at 0.97
- **THEN** the conversation waits on Ana Costa, `sure`, nothing is asked of the member, and nothing is sent

#### Scenario: Written to the member
- **WHEN** João writes to the inbox alone
- **THEN** the rule does not match and the need is raised as before

#### Scenario: Two people in To
- **WHEN** João writes to Ana Costa and Eva Lopes, copying the inbox
- **THEN** the rule answers nothing and Scout asks whose move it is

#### Scenario: A reply taught for the same sender
- **WHEN** another rule says João's mail needs a reply, and he writes to Ana Costa copying the inbox
- **THEN** the need is that Reply, basis taught, and no wait is raised
