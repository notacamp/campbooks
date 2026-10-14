## ADDED Requirements

### Requirement: Taught rules
The module SHALL store taught rules: deterministic condition → action pairs
for one inbox, with one to four conditions that must all hold and one to four
actions, at most one per slot (`stream`, `archive`, `read`, `needs_reply`,
`asks_action`, `surface`; `add_label` is additive). Conditions SHALL be
`sender_address is`, `sender_domain is` (never a free-mail domain on its
own), `list_id is`, `subject_template is`, `subject_contains` (whole words,
case- and accent-insensitive), `header is|present` (a stored header),
`label has` and `stream is`. Actions SHALL be `move_to_stream`, `archive`,
`add_label`, `mark_read`, `needs_reply` true or false, `asks_action` true or
false and `surface` always or never. A rule SHALL record its origin
(`action`, `onboarding`, `instruction`, `correction`, `suggestion`), the
member's words for an instruction, its author, and its state (`proposed`,
`active`, `paused`, `removed`). Taught rules are member data: the built-in
email signals stay the module's only rules of its own.

#### Scenario: A rule from a move
- **WHEN** Ana creates a rule with `sender_address is notifications@github.com` and `move_to_stream updates`
- **THEN** it is stored active for her inbox with origin `action` and Ana as its author

#### Scenario: A free-mail domain alone
- **WHEN** a rule's only condition is `sender_domain is gmail.com`
- **THEN** the response is 400 with code `invalid_rule` and nothing is stored

#### Scenario: Two actions in one slot
- **WHEN** a rule asks to move to `updates` and to `receipts`
- **THEN** the response is 400 with code `invalid_rule`

### Requirement: Subject templates
Every message SHALL keep a subject template: the subject lower-cased,
without `Re:`/`Fwd:` prefixes in the four locales, with dates and digit runs
replaced by `#`, tokens of six or more characters mixing letters and digits
replaced by `*`, spaces collapsed and at most 120 characters, or none when
fewer than two literal words remain. Existing messages SHALL be backfilled.

#### Scenario: An order notice
- **WHEN** a message's subject is "Re: Your order #48213 has shipped"
- **THEN** its template is "your order # has shipped"

#### Scenario: Nothing to keep
- **WHEN** a subject is "Invoice 2026-10-03"
- **THEN** it has no template

### Requirement: Rules run first and always the same
For each new inbound message, the module SHALL evaluate the inbox's active
rules after the built-in signals and before placing the conversation. Per
slot, the matching rule with the highest specificity (template, List-Id or
header 3; subject words or sender address 2; domain, label or stream 1;
summed) SHALL win, then the latest confirmed, then the lowest id. A rule's
answers to `stream`, `needs_reply` and `asks_action` SHALL be stored in
`decisions` with source `taught`, the rule as the reason and `acted = true`,
and SHALL override the signals' answers to the same question; the model pass
SHALL never override them. Its `archive`, `mark_read` and `add_label`
actions SHALL apply to the new message. A stream placed by a rule SHALL have
stream source `taught`, which a member's move overrides and nothing else
does. The same message and the same rules SHALL always give the same outcome.

#### Scenario: A new notification
- **WHEN** a message from notifications@github.com arrives and an active rule moves that sender's mail to `updates`
- **THEN** the new conversation is in `updates` with stream source `taught`, and the stream decision has source `taught` and acted

#### Scenario: The more specific rule
- **WHEN** one rule moves mail from shop.example to `receipts` and another moves "your order # has shipped" from shop.example to `updates`
- **THEN** a shipping notice from shop.example goes to `updates` and an invoice from it goes to `receipts`

#### Scenario: A rule beats the signals
- **WHEN** the signals answer `needs_reply` true at 0.95 and an active rule answers it false for that sender
- **THEN** no reply need is raised

### Requirement: Suspicious mail is never hidden by a rule
When a message has a true `suspicious` answer from any source, or its
conversation has an open Review need, rules SHALL NOT archive it, mark it
read, move it out of `primary`, answer a need question false, or apply
`surface: never`; the Review need SHALL stay. Such a conversation SHALL
never start a rule suggestion.

#### Scenario: A look-alike sender
- **WHEN** a phishing message claims to be from notifications@github.com and the rules answer `suspicious` true
- **THEN** the archive and move rules for that sender leave it in Primary with its Review need

### Requirement: Surfacing
A rule with `surface: never` SHALL keep a need from being raised for the
mail it matches, and SHALL clear an open one with cause `rule`. A rule with
`surface: always` SHALL keep any other rule from archiving, marking read or
moving the conversation out of `primary`; it raises no need of its own in
this version.

#### Scenario: Never needs me
- **WHEN** a bill from billing@hosting.example arrives and a rule says `surface: never` for that sender
- **THEN** no need is raised for it

#### Scenario: Always show me
- **WHEN** one rule archives mail from mfiscal.example and a more specific one says `surface: always` for ines@mfiscal.example
- **THEN** a message from ines@mfiscal.example stays in the inbox, in Primary

### Requirement: Rule operations
`POST …/inboxes/{inbox_id}/rules/preview` (`conversations.preview_rule`,
`conversations:read`, not mutating: a read served by POST so the rule's
addresses and words stay out of URLs and access logs) SHALL answer, for
conditions and actions, how many conversations in the inbox the rule
matches and would change, up to five of them (subject, sender and date,
newest first), how many it would skip as suspicious or placed by a member,
and the active rules it would take over from with how many conversations.
`POST …/inboxes/{inbox_id}/rules` (`conversations.create_rule`,
`conversations:write`, undo `conversations.remove_rule`) SHALL store a rule
active or proposed and, when `apply_to_existing` is set for an active rule,
start a run. `POST …/rules/{rule_id}/confirm` (`conversations.confirm_rule`)
SHALL make a proposed rule active (with `apply_to_existing`), else answer 409
`rule_not_proposed`. `PATCH …/rules/{rule_id}` (`conversations.update_rule`)
SHALL change its conditions or actions, pause it, or let a manager adopt it.
`DELETE …/rules/{rule_id}` (`conversations.remove_rule`) SHALL remove it, and
with `restore=true` put back what it changed. `GET …/rules`
(`conversations.list_rules`, proposed rules first) and
`GET …/rules/{rule_id}` (`conversations.get_rule`) SHALL list the rules of
the inboxes the caller can see with their counts, latest run and recent
exceptions. A rule with the same conditions and an action of the same kind
as an active one SHALL replace it.

#### Scenario: Preview
- **WHEN** a member previews moving mail from notifications@github.com to `updates` in an inbox with 1,000 such conversations in Primary, 2 of them suspicious
- **THEN** the preview would change 998, lists five of them and says 2 are skipped as suspicious

#### Scenario: A proposed rule does nothing
- **WHEN** a rule is created as `proposed`
- **THEN** new mail it matches is not changed until a member confirms it

#### Scenario: Replacing
- **WHEN** a member creates a rule moving notifications@github.com to `newsletters` while one moves it to `updates`
- **THEN** the old rule is removed without putting mail back and the new one is active

### Requirement: Who may teach
Previewing, creating and confirming a rule SHALL take the `sender` role on its
inbox (a `reader` gets 403 `inbox_read_only`). Updating and removing it SHALL
take its author while they hold `sender`, or a `manager` of the inbox; others
get 403 `rule_not_yours`. A proposed rule MAY be turned down by anyone who may
teach in its inbox. Reading rules SHALL take any role on the inbox; a rule in
an inbox the caller cannot see SHALL answer 404. A client token SHALL only
read and preview rules: creating, confirming, changing or removing one
answers 403 `rule_needs_member`. A rule SHALL act for every member of its
inbox. When its author no longer holds `sender` on the inbox, the rule SHALL
be paused until a manager adopts or removes it.

#### Scenario: Leo's rule in a shared inbox
- **WHEN** Leo taught a rule in info@ and Ana, a sender there, tries to remove it
- **THEN** the response is 403 `rule_not_yours`, and Ana's manager colleague can remove it

#### Scenario: The author leaves
- **WHEN** Leo is removed from info@
- **THEN** his rules there are paused and stop acting on new mail

#### Scenario: An app
- **WHEN** a client token with `conversations:write` creates a rule
- **THEN** the response is 403 with code `rule_needs_member`

### Requirement: Rules from evidence ask first
A rule created with origin `correction` or `suggestion` SHALL be active only
when the kernel's `MemberPreferences` port says the creating member lets
Campbooks act without asking and the inbox has no other member; the
response SHALL then say `acted_without_asking`. Otherwise it SHALL be
proposed. Until a module installs the port, it SHALL answer "ask first".
Rules made by hand, from an action or confirmed by a member SHALL not ask the
port.

#### Scenario: The default
- **WHEN** a rule is created from corrections and no preferences module is installed
- **THEN** it is proposed

#### Scenario: Act and tell me
- **WHEN** the port says Sam lets Campbooks act and Sam's inbox is his alone
- **THEN** the rule is active and the response says `acted_without_asking`

#### Scenario: A shared inbox
- **WHEN** the port says Sam lets Campbooks act but Leo also uses the inbox
- **THEN** the rule is proposed

### Requirement: Applying a rule to existing mail
Creating or confirming an active rule with `apply_to_existing` SHALL start
an apply run, `conversations.apply_rule`, that changes the matching
conversations of the inbox in batches of 100, each batch in one transaction,
skipping suspicious conversations, conversations a member placed or changed
back, and those already so, recording each change with what it was before.
The run SHALL be resumable from its stored position and SHALL never change a
conversation twice for one rule. It SHALL re-check before each batch that
the rule is active and its author may change the inbox, and stop otherwise.
Each change SHALL publish `conversations.conversation_changed` and
`conversations.changes_requested` as the operations do; each batch SHALL
publish `conversations.rule_changed` with the run's total and done.

#### Scenario: Moving 1,000
- **WHEN** a rule moving notifications@github.com to `updates` is created with `apply_to_existing` and 1,000 lookalikes sit in Primary
- **THEN** ten batches move them, `rule_changed` reports 100, 200, … 1,000 done, and the connector receives a move for each

#### Scenario: A crash halfway
- **WHEN** the worker stops after 340 conversations and the job is retried
- **THEN** the run continues from the 341st and no application is recorded twice

### Requirement: Rule events
Every change to a rule or its run SHALL publish `conversations.rule_changed`
v1 carrying the
workspace, inbox and rule ids, the rule's state, and the current run's kind,
state, total and done; never its conditions, the member's words or any
address. It SHALL declare an upsert of `conversations.rule` within its inbox.

#### Scenario: Someone outside the inbox
- **WHEN** a rule in info@ is applied and a member who is not in info@ follows the change feed
- **THEN** they receive none of its changes

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
conversation, and never while the member dismissed it.
`POST …/conversations/{conversation_id}/rule-suggestions/dismiss`
(`conversations.dismiss_rule_suggestion`, with the suggestion's `key`) SHALL
hide it from that member for 30 days, and for good after a second dismissal.

#### Scenario: The 1,000 notifications
- **WHEN** Ana moves one GitHub notification to Updates and 1,000 more from notifications@github.com are in Primary
- **THEN** the suggestion's default is "sender is notifications@github.com" with 1,000, followed by the acme/web List-Id with 612 and the sender with the subject's template

#### Scenario: A person
- **WHEN** Ana archives a message from Marta, whom her inbox has written to
- **THEN** there is no suggestion

#### Scenario: Just this one
- **WHEN** Ana dismisses the suggestion and moves another GitHub notification the next day
- **THEN** there is no suggestion

### Requirement: Exceptions
When a member changes back what a rule did to a conversation (moves it to
another stream, unarchives it, marks it unread or removes the label), the
module SHALL mark that application excepted and the rule SHALL never change
that conversation again. The responses of the operations that return the
conversation (`move_to_stream`, `unarchive`, `mark_unread`) SHALL carry the
rule and how many exceptions it had in the last 30 days.

#### Scenario: A security alert moved back
- **WHEN** Ana moves a rule-moved GitHub security alert back to Primary
- **THEN** the response names the rule with one recent exception, and the next message in that conversation stays in Primary

### Requirement: Undoing a rule
`conversations.remove_rule` with `restore=true` SHALL cancel any apply run
of the rule and start a revert run that puts every changed conversation back
as it was before, except those excepted or changed by a member since, with
the same batching, progress and events as an apply run.

#### Scenario: Undo all
- **WHEN** Ana presses Z right after 998 conversations were moved
- **THEN** the rule is removed and the 998 are back in Primary, except one Leo has since starred and moved himself

### Requirement: Rules in summaries
A conversation summary SHALL carry `rule` (its id, the action it took, and
how many conversations it has changed) when a rule placed or changed the
conversation and no member changed it back.

#### Scenario: Placed by a rule
- **WHEN** a member lists Updates after a rule moved a new notification there
- **THEN** its summary names the rule and its count

### Requirement: Purging rules
Removing an inbox's mail and purging a workspace SHALL delete its rules,
runs, applications and dismissals.

#### Scenario: Disconnecting with the mail deleted
- **WHEN** the inbox's connection is removed and its mail deleted
- **THEN** its rules are gone too
