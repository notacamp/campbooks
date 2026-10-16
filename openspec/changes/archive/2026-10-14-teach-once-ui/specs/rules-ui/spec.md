## ADDED Requirements

### Requirement: The rules as sentences
The conversations client module SHALL export `RulesList`, for the rules slot
of Scout's memory, listing `conversations.list_rules` as one sentence each,
built from the rule's conditions and actions in the user's language ("Mail
from notifications@github.com about “pull request” → Updates"): rules waiting
for a yes first under "Waiting for you", then each inbox's under its address,
each with where it came from (an action, onboarding answers, the user's own
words quoted, corrections, a suggestion), how many conversations it changed,
when it last acted, how many the user changed back, "For everyone" in a
shared inbox, and "Paused" when its author can no longer change the inbox.
With no rule it SHALL say how rules come about.

#### Scenario: Two inboxes and a proposal
- **WHEN** a proposed rule from corrections and an active rule in Sam's inbox are listed
- **THEN** the proposal comes first under "Waiting for you", and the rule under Sam's address with "40 so far"

### Requirement: Keeping, turning down and removing a rule
A proposed rule SHALL offer Keep this rule (`conversations.confirm_rule`) and
No (`conversations.remove_rule`). Any other rule the user may change SHALL
offer Remove, which confirms in place and, when the rule changed mail,
offers to put it back (the default) or leave it, then calls
`conversations.remove_rule` with `restore` accordingly.

#### Scenario: Remove and put back
- **WHEN** the user removes a rule that moved 40 conversations, keeping "Also put back the 40 it changed"
- **THEN** `remove_rule` is called with `restore=true`
