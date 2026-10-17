# rules-ui Specification

## Purpose
The rules a member taught Campbooks, shown as plain sentences they can keep, turn down or remove: the list the Scout's memory page shows in its rules slot.

## Requirements

### Requirement: The rules as sentences
The conversations client module SHALL export `RulesList`, for the rules slot
of Scout's memory, listing `conversations.list_rules` as one sentence each,
built from the rule's conditions and actions in the user's language ("Mail
from notifications@github.com about “pull request” → Updates"): rules waiting
for a yes first under "Waiting for you", then each inbox's under its address,
each with where it came from (an action, onboarding answers, the user's own
words quoted, corrections, a suggestion), how many conversations it changed,
when it last acted, how many the user changed back, "For everyone" in a
shared inbox, and, when it is paused, first on its line, why, from its `paused_reason`: its
author can no longer change the inbox, its author is away for now (it acts
again on its own; the list never says why the author is away), a member
paused it, the inbox is kept read-only, or it names a whole group of sites (a public suffix) and so matches nothing. With no rule it SHALL say how
rules come about.

#### Scenario: Two inboxes and a proposal
- **WHEN** a proposed rule from corrections and an active rule in Sam's inbox are listed
- **THEN** the proposal comes first under "Waiting for you", and the rule under Sam's address with "40 so far"

#### Scenario: A rule whose author is away
- **WHEN** a rule is listed with `state` `paused` and `paused_reason` `author_unavailable`
- **THEN** it is muted and its line starts "Paused for now; it acts again on its own", naming no cause

#### Scenario: A rule naming a public suffix
- **WHEN** a rule is listed with `state` `paused` and `paused_reason` `domain_too_broad`
- **THEN** its line starts "Paused: its domain is too broad" and Remove is offered

### Requirement: Keeping, turning down and removing a rule
A proposed rule SHALL offer Keep this rule (`conversations.confirm_rule`) and
No (`conversations.remove_rule`). When its `paused_once_kept` is
`author_unavailable`, the row SHALL say before a yes, in a neutral status
line naming no cause, that once kept it stays paused until whoever taught it
is back or someone takes it over, and the toast after keeping it SHALL say it
is kept and paused for now. Any other rule the user may change SHALL
offer Remove, which confirms in place and, when the rule changed mail,
offers to put it back (the default) or leave it, then calls
`conversations.remove_rule` with `restore` accordingly.

#### Scenario: Remove and put back
- **WHEN** the user removes a rule that moved 40 conversations, keeping "Also put back the 40 it changed"
- **THEN** `remove_rule` is called with `restore=true`

#### Scenario: Keeping a proposal whose author is away
- **WHEN** a proposal with `paused_once_kept` `author_unavailable` is listed next to another proposal, and the user keeps it
- **THEN** only that row says "If kept, it stays paused until whoever taught it is back or someone takes it over", nothing says why, and the toast reads "Rule kept, paused for now"
