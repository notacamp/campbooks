## ADDED Requirements

### Requirement: Needs for mail stored before attention
When the module is upgraded to a version with needs, every inbox that
existed SHALL have its needs worked out once, without any manual step: each
conversation whose last message was sent within
`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS`, and each conversation with an
open need, SHALL be refreshed as live mail is, from the messages and
decisions already stored, in the conversation's workspace, a page at a time.
No AI role and no rules strategy SHALL be asked. Each change SHALL publish
`conversations.need_changed` as for live mail, and running it again SHALL
change nothing. A conversation whose need cannot be worked out SHALL be
skipped without undoing the others, and an inbox whose page keeps failing
SHALL be recorded as failed at its cursor, never left running. An inbox kept
read-only after its connection was removed SHALL be left as it is.

#### Scenario: Mail stored before the upgrade
- **WHEN** an inbox holds a question from Marta answered by the rules before the upgrade, and the upgraded server starts
- **THEN** within minutes the conversation has its `reply` need, published as `conversations.need_changed`, and no model or rules call is made

#### Scenario: Old mail
- **WHEN** an inbox holds a question sent 40 days before the upgrade
- **THEN** no need is raised for it

#### Scenario: Already handled
- **WHEN** before the upgrade a member replied to one conversation, archived another that asked for a reply, and trashed a bill
- **THEN** none of them gets a need, and an archived request keeps its `do` need

#### Scenario: Twice
- **WHEN** the backfill runs again over the same inboxes
- **THEN** no need changes and no event is published

#### Scenario: Another workspace
- **WHEN** one workspace's inboxes are backfilled
- **THEN** no other workspace's needs or events change

#### Scenario: A kept read-only inbox
- **WHEN** an inbox's connection was removed and its mail kept read-only before the backfill runs
- **THEN** none of its needs changes and no event is published for it

#### Scenario: One conversation that cannot be worked out
- **WHEN** one conversation's stored answers no longer decode
- **THEN** it is skipped, the inbox's other conversations get their needs, and the inbox's backfill finishes

#### Scenario: A question its answers no longer support
- **WHEN** an inbox has an open `check` need whose message's accepted answers say it asks nothing
- **THEN** the backfill clears it with cause `superseded`
