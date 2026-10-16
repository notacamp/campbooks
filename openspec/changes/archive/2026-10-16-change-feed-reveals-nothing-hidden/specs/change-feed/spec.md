## ADDED Requirements

### Requirement: Nothing about what the caller may not see
What a stream sends SHALL depend only on the changes its caller may see and
on time: every frame's event, SSE `id` and data, and when heartbeats are
sent, SHALL be the same whether or not changes the caller may not see are
recorded (in an inbox they are not in, private to another person, or
written for client tokens only). Resume tokens SHALL be opaque: sealed by
the server so a client can neither read nor alter one, of one length, and
new on every frame. The stream SHALL NOT send the workspace's sequence
numbers or its head in any form. A client resuming SHALL receive the same
outcome (`ready` and the same visible changes, or the same `reset`) whether
or not such changes were recorded.

#### Scenario: Mail in an inbox the member is not in
- **WHEN** a member follows the feed and the workspace's other inbox, which they are not in, receives mail
- **THEN** the member's stream (its frames, ids, tokens' shape and heartbeat times) is the same as in a workspace where that inbox received nothing

#### Scenario: Every kind of caller
- **WHEN** a member, an admin, a client acting for a member and a client acting for nobody each follow the feed while changes they may not see are recorded (another member's mail, draft or connection, a client's draft)
- **THEN** each one's stream matches its stream in a workspace without those changes, and resuming from any token it received replays the same frames

#### Scenario: Probing with made-up cursors
- **WHEN** a client resumes with a number, a token of another workspace or an altered token
- **THEN** it receives `reset` with reason `unknown_cursor`, whatever the workspace's head

## MODIFIED Requirements

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
token the server cannot open for the workspace (a number from an older
version, another workspace's, altered) SHALL produce one `reset` frame with
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

### Requirement: Retention
Changes SHALL be kept for a configured retention period (72 hours by
default) after they were numbered and deleted afterwards, and a
workspace's changes SHALL be deleted when the workspace is purged.

#### Scenario: Pruned
- **WHEN** a change was numbered longer ago than the retention period and the prune job runs
- **THEN** it is deleted, and resuming with a token from before it produces `reset`
