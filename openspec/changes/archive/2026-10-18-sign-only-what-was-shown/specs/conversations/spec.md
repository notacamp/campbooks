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
the response is for, the object's kind and id, and for a draft its inbox
id, its version and the same content hash `send_draft` binds into the send
request, for a
rule its binding generation, the same hash of its conditions and actions its
MAC binds, its inbox, its stored state and its author. The token SHALL NOT
be stored. Without an instance secret no token SHALL be issued.

`conversations.send_draft`, `conversations.update_draft` (every save: a
save keeps fields it cannot send, such as the inbox, the reply headers and
the attachments' files), `conversations.confirm_rule` (a proposal's yes and
a yes again) and `conversations.update_rule` with `adopt` (unless the
request sets both the conditions and the actions) SHALL take the token
back. After every access check and under the row lock, before anything
changes, the server SHALL recompute the hash of the row as stored and verify
the token against it in constant time. A missing token, one issued to
another member, or one that does not hold for the row as it is now SHALL
answer 409 `content_changed`, with no state change, nothing saved or
signed, no job queued and nothing published, and a WARN line carrying ids
only. A save that holds SHALL issue the next token over the draft as it
left it. Without an instance secret saves SHALL NOT be checked (nothing can
be signed) and `send_draft` SHALL refuse. Requests
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

#### Scenario: Hidden fields changed before an autosave
- **WHEN** Sam's composer read his draft, then SQL changes an attachment's file name, the quoted text or the reply headers, and the composer autosaves with the token it read
- **THEN** the save answers 409 `content_changed` and writes nothing, and a send with that token signs nothing

#### Scenario: A stale token after an autosave
- **WHEN** Sam's composer saves, then saves again with the token of before the first save
- **THEN** the second save answers 409 `content_changed`; with the first save's token it is saved

#### Scenario: Another draft's or rule's token
- **WHEN** a save, a send or a yes carries the token of another draft or rule of the same member
- **THEN** the answer is 409 `content_changed`

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
