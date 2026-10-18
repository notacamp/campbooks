## ADDED Requirements

### Requirement: Send and confirm sign only what the member was shown
Every response that shows a member a draft (`get_draft`, `list_drafts`,
`create_draft`, `update_draft`, `send_draft`, `cancel_send`) or a taught
rule (`get_rule`, `list_rules`, `create_rule`, `confirm_rule`,
`update_rule`, `remove_rule`, `restore_rule`, `decline_rule`) SHALL carry a
`content_token`: a version byte and an HMAC-SHA256 under the binding key
(derived from the instance secret, never stored in the database, logged or
sent), with its own context label
(`campbooks/conversations.content-token/v1`), over the workspace, the member
the response is for, the object's kind and id, and for a draft its version
and the same content hash `send_draft` binds into the send request, for a
rule its binding generation, the same hash of its conditions and actions its
MAC binds, its inbox, its stored state and its author. The token SHALL NOT
be stored. Without an instance secret no token SHALL be issued.

`conversations.send_draft`, `conversations.confirm_rule` (a proposal's yes
and a yes again) and `conversations.update_rule` with `adopt` (unless the
request sets both the conditions and the actions) SHALL take the token
back. After every access check and under the row lock, before anything
changes, the server SHALL recompute the hash of the row as stored and verify
the token against it in constant time. A missing token, one issued to
another member, or one that does not hold for the row as it is now SHALL
answer 409 `content_changed`, with no state change, nothing signed, no job
queued and nothing published, and a WARN line carrying ids only. Requests
that send the whole content they sign (`create_rule`, an `update_rule` with
both conditions and actions) and requests that keep content whose rule MAC
verifies under the lock (pausing, resuming, removing, bringing back) SHALL
NOT need a token.

#### Scenario: A Bcc added with SQL after the draft was shown
- **WHEN** Sam's composer read his draft, then a transaction of the runtime role adds a Bcc or changes the body without bumping its version, and Sam sends with the version and token he read
- **THEN** the answer is 409 `content_changed`, the draft is still a draft with no send request bound, no release is queued and no `conversations.send_requested` is published

#### Scenario: Read again, it is what the member saw
- **WHEN** Sam's client reads that draft again (it shows the Bcc) and Sam sends with the new token
- **THEN** it is scheduled

#### Scenario: A stale token after a save
- **WHEN** Sam saves the draft and sends with the token of before the save
- **THEN** the answer is 409 `content_changed`; with the save's token it is scheduled

#### Scenario: Another member's token
- **WHEN** a send or a yes carries a token issued to another member for the same content
- **THEN** the answer is 409 `content_changed`

#### Scenario: A proposal changed with SQL after it was shown
- **WHEN** Sam's client read a rule waiting for a yes, then SQL changes its actions, conditions or author, and Sam says yes with the token he read
- **THEN** the answer is 409 `content_changed`, the rule is still proposed and unsigned, no run starts, and nothing is asked of the provider

#### Scenario: Adopting what was shown
- **WHEN** a manager adopts a rule with a missing token, or one of before the rule was signed again
- **THEN** the answer is 409 `content_changed`; with the token of the rule as it is now, or with the whole rule in the request, it is adopted

#### Scenario: A composer opened before the upgrade
- **WHEN** a member sends from a composer that has no token
- **THEN** the answer is 409 `content_changed`, nothing is sent, and the next send with the token of the draft read again is scheduled
