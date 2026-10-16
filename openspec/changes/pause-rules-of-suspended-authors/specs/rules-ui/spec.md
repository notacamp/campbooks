## MODIFIED Requirements

### Requirement: The rules as sentences
The conversations client module SHALL export `RulesList`, for the rules slot
of Scout's memory, listing `conversations.list_rules` as one sentence each,
built from the rule's conditions and actions in the user's language ("Mail
from notifications@github.com about “pull request” → Updates"): rules waiting
for a yes first under "Waiting for you", then each inbox's under its address,
each with where it came from (an action, onboarding answers, the user's own
words quoted, corrections, a suggestion), how many conversations it changed,
when it last acted, how many the user changed back, "For everyone" in a
shared inbox, and, when it is paused, why, from its `paused_reason`: its
author can no longer change the inbox, its author is suspended, a member
paused it, or the inbox is kept read-only. With no rule it SHALL say how
rules come about.

#### Scenario: Two inboxes and a proposal
- **WHEN** a proposed rule from corrections and an active rule in Sam's inbox are listed
- **THEN** the proposal comes first under "Waiting for you", and the rule under Sam's address with "40 so far"

#### Scenario: A suspended author's rule
- **WHEN** a rule is listed with `state` `paused` and `paused_reason` `author_suspended`
- **THEN** it is muted and reads "Paused while whoever taught it is suspended"
