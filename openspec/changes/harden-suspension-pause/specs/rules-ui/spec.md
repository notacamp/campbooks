## MODIFIED Requirements

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
