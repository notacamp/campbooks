# change-feed Specification

## Purpose
How clients learn what changed in a workspace, resume after a disconnect,
and keep their cache current without refetching everything (ADR 0011).

## Requirements

### Requirement: Declared changes
A domain event SHALL be able to declare the records its change touched, each
as a record reference and an operation (`upsert` or `delete`), carrying ids
only. Publishing such an event SHALL record its changes in the same
transaction as the event, so a rolled-back change leaves no change behind.

#### Scenario: Rolled back
- **WHEN** a module publishes an event declaring changes and its transaction rolls back
- **THEN** no change is recorded and no subscriber receives one

### Requirement: Gap-free cursors
Every recorded change SHALL receive a cursor that is unique and increasing
within its workspace, and a change SHALL receive its cursor only after every
change of that workspace with a smaller cursor has committed. Cursors SHALL
remain valid across server restarts and between server replicas.

#### Scenario: Out-of-order commits
- **WHEN** two transactions in one workspace record changes and the later-started one commits first
- **THEN** a subscriber that has received cursor N never later receives a change with a cursor below N

#### Scenario: Producers are not delayed
- **WHEN** one transaction that recorded changes stays open while others in the same workspace record changes and commit
- **THEN** the others commit without waiting for it or for the numbering, and once it commits every change has a cursor with no gap

#### Scenario: Bursts coalesce
- **WHEN** a thousand events with changes are published in one workspace in a burst
- **THEN** a handful of numbering runs at most handle them, and their cursors are 1 to 1000 in publish order

#### Scenario: Another replica
- **WHEN** a client resumes with a cursor obtained from one server process against another process of the same instance
- **THEN** it receives exactly the changes after that cursor

### Requirement: Change stream endpoint
`GET /api/v1/w/{workspace_id}/changes` SHALL stream the workspace's changes
as Server-Sent Events to a caller with scope `workspaces:read` who belongs to
the workspace. It SHALL answer 401 without a valid token, 403 without the
scope and 404 to anyone who may not see the workspace. The web app SHALL
reach the same stream through `GET /bff/api/v1/w/{workspace_id}/changes`
with its session.

#### Scenario: Non-member
- **WHEN** a signed-in user who is not a member requests the stream
- **THEN** the response is 404 and no change is sent

#### Scenario: Through the BFF
- **WHEN** the web app opens `/bff/api/v1/w/{ws}/changes` with a live session
- **THEN** it receives the same frames as a bearer-token client of `/api/v1/w/{ws}/changes`

### Requirement: Frames
Each change frame SHALL carry its cursor as the SSE `id` and a JSON body of
exactly `cursor`, `record` (`type`, `id`), `op` and `version`. Frames SHALL
NOT carry record contents or personal data (names, email addresses,
message text).

#### Scenario: A member joins
- **WHEN** a user accepts an invitation to the workspace
- **THEN** every subscribed member receives a frame with record `workspaces.member` and the new member's user id, op `upsert`, and no name or email

### Requirement: Resuming
A client SHALL be able to resume with the last cursor it received, through
the `Last-Event-ID` header or the `after` query parameter, and SHALL then
receive every visible change after that cursor, in cursor order, before live
changes. Without a cursor the stream SHALL start at the current head. A
cursor that is unknown, older than the retained changes or more than the
configured backlog behind SHALL produce one `reset` frame carrying the
current head, after which the stream continues from it.

#### Scenario: Resume after a disconnect
- **WHEN** a client disconnects after cursor 5, changes 6 to 9 are recorded, and it reconnects with `Last-Event-ID: 5`
- **THEN** it receives 6, 7, 8 and 9 in order, then live changes

#### Scenario: Expired cursor
- **WHEN** a client resumes with a cursor whose changes were pruned
- **THEN** it receives a `reset` frame with the current head and then live changes

### Requirement: Liveness and connection lifetime
The stream SHALL send a heartbeat at a configured interval while idle, and
SHALL end cleanly with a `closing` frame after a configured maximum
connection time, never later than the caller's access token expires, so the
client reconnects with its cursor and is authorized again. Whenever the
stream resolves the caller's membership again (at least once a heartbeat
interval, busy or idle), it SHALL also ask whether the caller's token still
stands (the API's token-revocation check); when it no longer does (the
account was suspended or signed out everywhere), the stream SHALL send
`revoked` and end, and reconnecting with that token SHALL answer 401.

#### Scenario: Idle stream
- **WHEN** no change is recorded for longer than the heartbeat interval
- **THEN** the client receives a heartbeat

#### Scenario: Token about to expire
- **WHEN** the caller's access token expires before the maximum connection time
- **THEN** the stream sends `closing` and ends no later than the expiry

#### Scenario: The caller's tokens are revoked
- **WHEN** the account of a person following a workspace's changes is suspended or signs out everywhere while the stream is open
- **THEN** within one heartbeat interval the stream sends `revoked` and ends, and opening it again with the same token answers 401

### Requirement: Access filtering
The stream SHALL deliver a change only if the caller holds a scope of at
least one read operation that returns that record type, and, for a change
inside a container (an inbox, a calendar, a folder), only if the
container-visibility port admits the caller. Filtered changes SHALL still
advance the caller's cursor. When a change shows the caller may have lost
access to the workspace, the stream SHALL check membership again and, if it
is gone, send `revoked` and end.

#### Scenario: Missing scope
- **WHEN** a client token without any scope that reads `workspaces.member` subscribes and a member joins
- **THEN** it receives no frame for that change, and its next frame's cursor is past it

#### Scenario: Removed mid-stream
- **WHEN** a subscribed member is removed from the workspace
- **THEN** their stream sends `revoked` and ends, and reconnecting answers 404

### Requirement: Backpressure and limits
The stream SHALL read changes in bounded pages, reading the next page only
once the previous one has been sent, and SHALL hold no database connection
while waiting. The server SHALL limit open streams per caller and answer 429
beyond the limit.

#### Scenario: Many subscribers
- **WHEN** many clients subscribe to the same workspace and one change is recorded
- **THEN** every client receives it once

### Requirement: Retention
Changes SHALL be kept for a configured retention period (72 hours by
default) and deleted afterwards, and a workspace's changes SHALL be deleted
when the workspace is purged.

#### Scenario: Pruned
- **WHEN** a change is older than the retention period and the prune job runs
- **THEN** it is deleted and resuming from before it produces `reset`

### Requirement: Workspaces publish their changes
The `workspaces` module SHALL publish `workspaces.workspace_updated` when a
workspace's name or accent changes, carrying ids and the names of the
changed fields only. Its events SHALL declare changes: workspace created,
updated (upsert `workspaces.workspace`), member joined and role changed
(upsert `workspaces.member`), member left (delete `workspaces.member`) and
workspace deleting (delete `workspaces.workspace`).

#### Scenario: Rename
- **WHEN** an admin renames the workspace
- **THEN** subscribed members receive an `upsert` of `workspaces.workspace` with the workspace id, and the frame does not contain the new name

### Requirement: Client cache binding
The client SHALL key API queries as `[operationId, params]`, and SHALL map
each change to the invalidation of the queries whose operation reads the
change's record type, from a table generated from the OpenAPI document. It
SHALL resume with its last cursor after a disconnect, close the stream while
the page is hidden and resume when it is visible, invalidate every query of
the workspace on `reset`, and stop on `revoked` or 404.

#### Scenario: Member list refreshes
- **WHEN** the app has `workspaces.list_members` cached and a `workspaces.member` change arrives
- **THEN** that query is invalidated and refetched, and unrelated queries are not

### Requirement: Access is asked again on every page
An open stream SHALL decide container visibility afresh for every page of
changes it reads, never from an earlier page. It SHALL resolve the caller's
membership before showing a page that changes who belongs to the
workspace. When a container whose changes it has shown is no longer
visible, it SHALL send `reset` with reason `access_changed` (the client
drops what it cached and refetches) and show nothing more from that
container. Every heartbeat SHALL ask again too.

#### Scenario: Taken out of an inbox
- **WHEN** a member following the feed is removed from an inbox and new mail arrives in it
- **THEN** they receive `reset` (`access_changed`) and no id of the new mail

#### Scenario: Removed from the workspace
- **WHEN** a member following the feed is removed from the workspace
- **THEN** the stream sends `revoked` and ends, without the change that removed them

### Requirement: Private records reach their audience only
A change MAY name an audience: one user, one user and the workspace's
owners, admins and client tokens, or client tokens only. A stream SHALL show
such a change only to that audience. Draft changes SHALL reach only their
author (or client tokens, for a draft a client wrote). Connection changes
SHALL reach only the member who connected it, owners, admins and client
tokens.

#### Scenario: Another member's draft
- **WHEN** a sender of an inbox sends a draft
- **THEN** the inbox's other members and the workspace's admins do not receive its change
