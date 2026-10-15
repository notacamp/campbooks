## ADDED Requirements

### Requirement: A removed inbox leaves the index
On `conversations.inbox_removed`, the index SHALL forget every group in
that inbox.

#### Scenario: Disconnected and deleted
- **WHEN** an inbox is removed
- **THEN** none of its conversations is found by any caller, and other inboxes are untouched

### Requirement: Cursors and progress reveal nothing across scopes
A cursor SHALL be bound to its workspace as well as its caller and query,
and used in another workspace SHALL answer `cursor_mismatch`. Whether the
index is still being built (`indexing`) SHALL be reported only to a
caller who may see some container.

#### Scenario: The same caller, another workspace
- **WHEN** a member uses a cursor from workspace A in a search of workspace B
- **THEN** the answer is 400 `cursor_mismatch`
