## MODIFIED Requirements

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
- **WHEN** a rule asks to move to `updates` and to `receipts`
- **THEN** the response is 400 with code `invalid_rule`

#### Scenario: A stored rule naming a public suffix
- **WHEN** a rule stored with `sender_domain is github.io` meets mail from `a@a.github.io`
- **THEN** it does not match, while a rule for `github.com` still matches `a@mail.github.com`

### Requirement: Suspended authors
The module SHALL record, per workspace, whether each member is suspended,
from `workspaces.member_suspended` and `workspaces.member_unsuspended`
(`conversations.pause_suspended_author`,
`conversations.resume_unsuspended_author`), keeping for each member the
change with the newest `at`: a duplicate, or a change older than the one
recorded, SHALL change nothing. Whenever it records a change, the module
SHALL publish `conversations.rule_changed` for each of the member's rules
that is not removed. A member leaving the workspace SHALL be recorded as not
suspended as of their departure (they come back only by signing in).
`RuleView` SHALL carry `paused_reason` when its state is `paused`:
`inbox_read_only`, else `domain_too_broad` (a `sender_domain` naming a public
suffix), else `author_cannot_write`, else `paused_by_member`, else
`author_unavailable`. A suspension is an administrator's action and SHALL
not be disclosed to other members, clients or the assistant: no rule
reason, event or text readable in the workspace names it.

#### Scenario: Delivered out of order
- **WHEN** the lift (newer `at`) is delivered before the suspension it lifts
- **THEN** the member is recorded as not suspended and their rules stay active

#### Scenario: Delivered twice
- **WHEN** the same suspension is delivered twice
- **THEN** the member is suspended once and `rule_changed` is published once per rule

#### Scenario: Removed while suspended, invited back after
- **WHEN** a suspended member is removed from the workspace, the suspension is lifted (which no longer reaches that workspace), and they are invited back to the inbox
- **THEN** their rules act again
