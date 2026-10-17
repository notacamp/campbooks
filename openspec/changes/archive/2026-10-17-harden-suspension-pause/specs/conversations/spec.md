## MODIFIED Requirements

### Requirement: Suspended authors
The module SHALL record, per workspace, whether each member is suspended,
from `workspaces.member_suspended` and `workspaces.member_unsuspended`
(`conversations.pause_suspended_author`,
`conversations.resume_unsuspended_author`), keeping for each member the
newest change: the one with the higher `seq`; a change without `seq` is
older than one with it, and two without are ordered by `at`. A duplicate, or
a change older than the one recorded, SHALL change nothing. Whenever it
records a change, the module SHALL publish `conversations.rule_changed` for
each of the member's rules that is not removed. A member leaving the
workspace SHALL be recorded as not suspended as of their departure (they
come back only by signing in) unless the change recorded carries a `seq`,
which SHALL be kept: joining again announces the newest change.
`RuleView` SHALL carry `paused_reason` when its state is `paused`:
`inbox_read_only`, else `domain_too_broad` (a `sender_domain` naming a public
suffix), else `author_cannot_write`, else `paused_by_member`, else
`author_unavailable`; and, for a proposed rule, `paused_once_kept`: the
reason it would be paused once confirmed (confirming keeps its author). A
suspension is an administrator's action and SHALL not be disclosed to other
members, clients or the assistant: no rule reason, event or text readable in
the workspace names it.

#### Scenario: Delivered out of order
- **WHEN** the lift (higher `seq`) is delivered before the suspension it lifts
- **THEN** the member is recorded as not suspended and their rules stay active

#### Scenario: The clock stepped back
- **WHEN** a suspension with `seq` 1 is followed by its lift with `seq` 2 whose `at` is five minutes earlier, and both are delivered again
- **THEN** the rule is active, and `rule_changed` was published once for each change

#### Scenario: Before the counter
- **WHEN** a change without `seq` is recorded, then a counted lift with an older `at`, then a change without `seq` with a newer `at`
- **THEN** the counted lift wins and the last change is ignored

#### Scenario: Delivered twice
- **WHEN** the same suspension is delivered twice
- **THEN** the member is suspended once and `rule_changed` is published once per rule

#### Scenario: Removed while suspended, invited back after
- **WHEN** a suspended member is removed from the workspace, the suspension is lifted (which no longer reaches that workspace), and they are invited back to the inbox
- **THEN** their rules act again

#### Scenario: Back while still suspended
- **WHEN** a member whose counted suspension is recorded leaves, comes back to the inbox while still suspended, and is then lifted
- **THEN** their rule stays paused with `author_unavailable` until the lift, then acts

#### Scenario: Keeping a proposal whose author is away
- **WHEN** a member reads a proposed rule whose author is suspended, then confirms it
- **THEN** the proposal carries `paused_once_kept` `author_unavailable` and no `paused_reason`, and once confirmed it is the same author's, `paused` with `author_unavailable`
