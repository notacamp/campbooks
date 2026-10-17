## ADDED Requirements

### Requirement: A workspace's event alone is no proof
Any transaction working in a workspace can append that workspace's events,
so a consumer whose action reaches past the workspace (an account's ways in
and sessions, an app's refresh tokens, a grant at its provider) SHALL check
the state the event claims, through the owning module's port, before
acting, and SHALL do nothing while that state says otherwise. A port that
cannot answer SHALL make the delivery retry, never act on a guess.
`identity.forget_connected_way_in` (`connectors.connection_removed`) SHALL
act only when the connectors module no longer holds that connection for the
record's account; `identity.name_connected_mailbox`
(`connectors.connection_added`) SHALL name a record only with the
connection the connectors module holds for that grantor and provider
account; `identity.revoke_departed_consents` and
`connectors.revoke_departed_grants` (`workspaces.member_left`) SHALL act only
when the person is no longer a member of the workspace;
`identity.remove_workspace_clients` and `connectors.revoke_workspace_grants`
(`workspaces.workspace_deleting`) SHALL act only when the workspace is no
longer live. Consumers whose effects stay within the event's own workspace
(its own data, which the same transaction could change directly) SHALL NOT
be required to check.

#### Scenario: A forged removal of a live connection
- **WHEN** a transaction working in workspace A appends `connectors.connection_removed` for a connection the connectors module still holds, the only mailbox of a way in a connect created
- **THEN** the way in, its mailbox record and its sessions stay, and no `identity.way_in_removed` is published

#### Scenario: A forged connection names nothing
- **WHEN** `connectors.connection_added` names a connection the connectors module does not hold for its grantor
- **THEN** no mailbox record is named with it, and the connection it does hold still names the record

#### Scenario: A forged departure
- **WHEN** `workspaces.member_left` names someone who is still a member of that workspace
- **THEN** the apps they allowed there keep their consents and refresh tokens, and their connections there stay, unrevoked at the provider
- **AND** once they have left, the same event revokes them

#### Scenario: A forged deletion
- **WHEN** `workspaces.workspace_deleting` names a workspace that is still live
- **THEN** its clients, the consents bound to it and its connections stay, unrevoked at the provider
- **AND** once it is deleted, the same event removes and revokes them

## MODIFIED Requirements

### Requirement: Instance-wide rows change only in the kernel scope
A workspace transaction SHALL NOT read, update, delete or lock the
instance-wide rows (no workspace) of the event log and the job queue, nor
turn one of its own rows into an instance-wide one. It SHALL add an
instance-wide row only as a fresh event whose name, or a fresh delivery of
such an event to a consumer whose kind, is listed in
`kernel.instance_rows_from_workspaces` (pending, unclaimed, due now, keyed
by its event, no ordering key), and SHALL NOT enqueue any other
instance-wide job. The kernel SHALL publish such an event under an id it
chooses, reading nothing back. The list SHALL hold exactly the events declared
`DomainEvent::PUBLISHED_IN_WORKSPACES` and their consumers; the runtime role
SHALL only read it. Outside the kernel scope no row, a workspace's own
included, SHALL carry a schedule's unique key (`schedule:<name>`). The
kernel SHALL refuse an instance-wide job, or an instance-wide event not
declared `PUBLISHED_IN_WORKSPACES`, from a workspace transaction before
asking the database. A delivery SHALL be handled only when its event is
logged in the event's own workspace (none for an instance-wide type);
otherwise it SHALL fail for good without running its handler. Only the
kernel scope SHALL read or write the schedules' last ticks
(`kernel.schedules`). Only the kernel scope SHALL change instance-wide rows:
claiming, renewing, releasing, retrying, reaping and completing
instance-wide jobs, pruning and the admin's queue SHALL keep working there.

#### Scenario: Changing an instance-wide job from a workspace
- **WHEN** a transaction working in workspace A updates or deletes the queue's or the event log's instance-wide rows, or locks them with `FOR UPDATE`
- **THEN** no row is changed or locked, and the rows are unchanged once it commits

#### Scenario: Reading an account's events from a workspace
- **WHEN** a transaction working in workspace A selects the event log's `identity.signed_out_everywhere` and `identity.account_suspended` rows, any row naming an account, or the queue's instance-wide deliveries
- **THEN** it sees none of them, while the kernel scope sees them all

#### Scenario: Moving a row out of the workspace
- **WHEN** a transaction working in workspace A sets the workspace of one of its own jobs to none
- **THEN** the statement fails

#### Scenario: Forging a schedule's row
- **WHEN** a transaction working in workspace A inserts, as raw SQL, a job holding `schedule:kernel.prune_jobs`, instance-wide or of its own workspace
- **THEN** row-level security refuses it, and the schedule's next tick enqueues as usual

#### Scenario: Forging other instance-wide work
- **WHEN** a transaction working in workspace A inserts, as raw SQL, an instance-wide delivery due never, already claimed, ordered behind another key or keyed otherwise than by its event, a job of an unlisted kind, an event of an unlisted name (`identity.account_suspended`), or a listed event dated in the future
- **THEN** row-level security refuses each one

#### Scenario: An account's event forged in a workspace
- **WHEN** a transaction working in workspace A writes, as raw SQL into its own workspace, an `identity.signed_out_everywhere` naming another person's account and a `bff.end_signed_out_sessions` delivery of it, and the worker runs
- **THEN** the delivery fails for good, its handler never runs, and that person stays signed in

#### Scenario: An event naming another workspace
- **WHEN** a workspace's event whose payload names workspace B is logged in workspace A, with a delivery there
- **THEN** the delivery fails for good and its handler never runs

#### Scenario: The schedules' last ticks
- **WHEN** a transaction working in a workspace, or one that set nothing, reads `kernel.schedules`, sets a schedule's `last_tick` far ahead, or inserts a row
- **THEN** it sees no row, changes none, and the insert is refused

#### Scenario: Adding instance-wide work from a workspace
- **WHEN** a transaction working in workspace A publishes an instance-wide event declared `PUBLISHED_IN_WORKSPACES` (a member removes a workspace client; a departed member's consents are revoked; a removed mailbox takes the account's last way in)
- **THEN** the event and its deliveries are added under the id the publish returns, and the worker runs them

#### Scenario: Refused by the kernel
- **WHEN** a transaction working in workspace A enqueues an instance-wide job, or publishes an instance-wide event not declared `PUBLISHED_IN_WORKSPACES`
- **THEN** the kernel refuses it with an invalid-argument error and nothing is added

#### Scenario: The list matches the registry
- **WHEN** the server's registry is built and the migrated list is read
- **THEN** they name the same events and delivery kinds, and the runtime role may read the list but not change it

#### Scenario: Instance-wide jobs keep running
- **WHEN** an instance-wide job fails once, another's worker dies holding it, and a third fails for good, as the runtime role
- **THEN** the first is retried and succeeds, the second is reaped and run again, and the third is recorded as failed
