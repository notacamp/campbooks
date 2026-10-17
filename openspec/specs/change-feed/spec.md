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
Every recorded change SHALL receive a sequence number that is unique and
increasing within its workspace, and a change SHALL receive its number only
after every change of that workspace with a smaller number has committed,
so a stream reading in order never skips one. Sequence numbers stay inside
the server; clients receive resume tokens. A resume token SHALL remain
valid across server restarts and between server replicas of an instance
configured with a secret key.

#### Scenario: Out-of-order commits
- **WHEN** two transactions in one workspace record changes and the later-started one commits first
- **THEN** a subscriber that has received a change never later receives one numbered below it

#### Scenario: Producers are not delayed
- **WHEN** one transaction that recorded changes stays open while others in the same workspace record changes and commit
- **THEN** the others commit without waiting for it or for the numbering, and once it commits every change has a number with no gap

#### Scenario: Bursts coalesce
- **WHEN** a thousand events with changes are published in one workspace in a burst
- **THEN** a handful of numbering runs at most handle them, and they are numbered 1 to 1000 in publish order

#### Scenario: Another replica
- **WHEN** a client resumes with a token obtained from one server process against another process of the same instance
- **THEN** it receives exactly the changes after that token

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
Each change frame SHALL carry a resume token as the SSE `id` and a JSON
body of exactly `cursor` (the same token), `record` (`type`, `id`), `op` and
`version`. Each visible change SHALL be one frame. `ready`, `heartbeat` and
`reset` frames SHALL carry a resume token the same way; `ready` SHALL carry
nothing else. Frames SHALL NOT carry record contents or personal data
(names, email addresses, message text).

#### Scenario: A member joins
- **WHEN** a user accepts an invitation to the workspace
- **THEN** every subscribed member receives a frame with record `workspaces.member` and the new member's user id, op `upsert`, and no name or email

### Requirement: Resuming
A client SHALL be able to resume with the last resume token it received,
through the `Last-Event-ID` header or the `after` query parameter, and SHALL
then receive every visible change after that token, in order, before live
changes. Without a token the stream SHALL start at the current head. A
token the server cannot open for the workspace and caller (a number from an
older version, another workspace's or caller's, altered) SHALL produce one
`reset` frame with
reason `unknown_cursor`. A token older than the retention period less a
few minutes SHALL produce `reset` with reason `cursor_expired`; whether a
token has expired SHALL depend on its age alone, never on how many changes
were recorded after it. After a `reset` the stream continues from the
current head.

#### Scenario: Resume after a disconnect
- **WHEN** a client disconnects after a change, four more are recorded, and it reconnects with that change's token as `Last-Event-ID`
- **THEN** it receives the four in order, then live changes

#### Scenario: Expired cursor
- **WHEN** a client resumes with a token older than the retention period
- **THEN** it receives a `reset` frame with reason `cursor_expired` and then live changes

#### Scenario: A long backlog of hidden changes
- **WHEN** a client resumes after hundreds of changes it may not see and one it may see
- **THEN** it receives `ready` and the one change, never a `reset`

### Requirement: Liveness and connection lifetime
The stream SHALL send a `heartbeat` frame with a fresh resume token every
configured interval from when it opened, whether busy or idle, and SHALL
end cleanly with a `closing` frame after a configured maximum connection
time, never later than the caller's access token expires, so the client
reconnects with its token and is authorized again. Whenever the stream
resolves the caller's membership again (at least once a heartbeat
interval, busy or idle), it SHALL also ask whether the caller's token still
stands (the API's token-revocation check); when it no longer does (the
account was suspended or signed out everywhere), the stream SHALL send
`revoked` and end, and reconnecting with that token SHALL answer 401.

#### Scenario: Idle stream
- **WHEN** no change is recorded for longer than the heartbeat interval
- **THEN** the client receives a heartbeat

#### Scenario: Busy stream
- **WHEN** changes keep arriving for longer than the heartbeat interval
- **THEN** the client still receives a heartbeat every interval from the open

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
container-visibility port admits the caller. A heartbeat's token SHALL let
the client resume past the filtered changes the stream had read before it.
When a change shows the caller may have lost access to the workspace, the
stream SHALL check membership again and, if it is gone, send `revoked` and
end.

#### Scenario: Missing scope
- **WHEN** a client token without any scope that reads `workspaces.member` subscribes and a member joins
- **THEN** it receives no frame for that change, and resuming from its next heartbeat's token does not deliver it

#### Scenario: Removed mid-stream
- **WHEN** a subscribed member is removed from the workspace
- **THEN** their stream sends `revoked` and ends, and reconnecting answers 404

### Requirement: Backpressure and limits
The stream SHALL read changes in bounded pages of changes the caller may
see, reading the next page only once the previous one has been sent, and
SHALL hold no database connection while waiting. The server SHALL limit
open streams per caller and answer 429 beyond the limit, and SHALL limit how
many streams a caller opens per configured window (across processes),
answering 429 `rate_limited` beyond it, whatever happens in the workspace.
That window SHALL be between one second and a day, checked at startup.

#### Scenario: Many subscribers
- **WHEN** many clients subscribe to the same workspace and one change is recorded
- **THEN** every client receives it once

#### Scenario: Reconnecting in a loop
- **WHEN** a caller opens more streams than the configured number within the window
- **THEN** the next open answers 429 `rate_limited`, and other callers are not affected

#### Scenario: A window longer than a day
- **WHEN** `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` is 0 or 86 401
- **THEN** the server refuses to start, naming the setting

### Requirement: Retention
Changes SHALL be kept for a configured retention period (72 hours by
default) after they were numbered and deleted afterwards, and a
workspace's changes SHALL be deleted when the workspace is purged.

#### Scenario: Pruned
- **WHEN** a change was numbered longer ago than the retention period and the prune job runs
- **THEN** it is deleted, and resuming with a token from before it produces `reset`

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

### Requirement: Nothing about what the caller may not see
What a stream sends SHALL depend only on the changes its caller may see and
on time: every frame's event, SSE `id` and data, and when heartbeats are
sent, SHALL be the same whether or not changes the caller may not see are
recorded (in an inbox they are not in, private to another person, or
written for client tokens only). The stream SHALL read in pages of changes
the caller may see, filtered before the page is cut, so how its frames
group never depends on such changes. Resume tokens SHALL be opaque: sealed
by the server so a client can neither read nor alter one, of one length,
new on every frame, and bound to the workspace and the caller. The stream
SHALL NOT send the workspace's sequence numbers or its head in any form. A
client resuming SHALL receive the same outcome (`ready` and the same visible
changes in the same pages, or the same `reset`) whether or not such changes
were recorded.

#### Scenario: Mail in an inbox the member is not in
- **WHEN** a member follows the feed and the workspace's other inbox, which they are not in, receives mail
- **THEN** the member's stream (its frames, ids, tokens' shape and heartbeat times) is the same as in a workspace where that inbox received nothing

#### Scenario: Every kind of caller
- **WHEN** a member, an admin, a client acting for a member and a client acting for nobody each follow the feed while changes they may not see are recorded (another member's mail, draft or connection, a client's draft)
- **THEN** each one's stream matches its stream in a workspace without those changes, and resuming from any token it received replays the same frames

#### Scenario: Catching up past many hidden changes
- **WHEN** a member resumes after more changes they may not see than a page holds, interleaved with changes they may see
- **THEN** they receive the same frames, in the same pages, all before the next heartbeat, as in a workspace without the hidden changes

#### Scenario: Probing with made-up cursors
- **WHEN** a client resumes with a number, a token of another workspace or caller, or an altered token
- **THEN** it receives `reset` with reason `unknown_cursor`, whatever the workspace's head
