## MODIFIED Requirements

### Requirement: A way in that came only from connecting goes with its mailbox
When a connect creates a way in (`linked_via = connect`), the identity module
SHALL record, in the transaction that creates the way in, the workspace its
mailbox is connected in, and SHALL record every later workspace that way in's
mailbox is connected in before that grant is handed over; if the record
cannot be written, the round trip SHALL fail rather than hand the grant over.
The hand-over SHALL run to its end even if the browser goes away.
When the hand-over then fails (the connectors module refuses the grant, mail
access was declined, no refresh token came back), the record that round trip
wrote SHALL be taken back and, in the same transaction under the account
lock, a way in that round trip created SHALL be removed too (as below),
unless it is the account's last way in. Once the grant is accepted, the
record SHALL name the connection it became (written if it went missing).
The connectors module SHALL publish `connectors.connection_added` for every
accepted grant, with its `provider`, `account_subject` and the grantor's
account (`grantor_subject`); `identity.name_connected_mailbox`, consuming
it, SHALL name the record of that provider account's way in in that
workspace, and only when that way in belongs to the grantor's account and
is tracked (has a record), and the connectors module, asked through the
grant-intake port, holds that very connection as the grantor's connection
of that provider account in that workspace. Replaying the event SHALL change
nothing.
A record still naming no connection a day after it was written SHALL be
checked with the connectors module through the grant-intake port: when the
connection of that provider account in that workspace, granted by that
account, exists, the prune SHALL name the record with it and leave the way
in; when it does not, the prune SHALL delete the record (if it is still
unnamed and still that old) and, under the account lock, remove a way in a
connect created that it leaves with no record (as below, unless it is the
account's last), published with reason `mailbox_never_connected`. A record
the port could not answer for SHALL be left for a later run. The prune
SHALL settle one account per transaction, in account order. A connect
retried for a record still naming no connection SHALL write that record
afresh, so it is a day old only a day after the last attempt; a named
record SHALL keep the time it was written. Should the hand-over task fail
unexpectedly, the browser SHALL still go back with a `link_error`.
`identity.forget_connected_way_in`, consuming `connectors.connection_removed`
(its workspace, connection, `provider` and `account_subject`), SHALL first
ask the connectors module, through the grant-intake port, for the record's
account's connection of that provider account in that workspace, and do
nothing while that is still the named connection (or retry while the port
cannot answer); it SHALL then lock
the account, then act only on the record naming that very connection (a
record that names none is never acted on), forget it and, when that way in
has none left, was
linked by connecting, and is not the account's last way in (provider accounts
and passkeys), SHALL remove it as
`identity.remove_way_in` does: ending every session signed in with it,
revoking their refresh tokens, auditing it and publishing
`identity.way_in_removed`. A way in with connections that were never recorded
(linked before the record existed), one linked by signing up or by Add, and
the last way in SHALL stay. Replaying the event SHALL change nothing.

#### Scenario: Its last connection goes
- **WHEN** a way in a connect created has connections in two workspaces and both are removed
- **THEN** it stays after the first removal and is gone after the second, with one `identity.way_in_removed`

#### Scenario: Another member's disconnect
- **WHEN** B connects support@ in a workspace without signing in with it, A (who made support@ a way in by connecting it in another workspace) then connects the same account in B's workspace and the grant is refused, and B later disconnects their connection
- **THEN** A's way in is untouched, and no record of A's was left in B's workspace

#### Scenario: A removal announced for a live connection
- **WHEN** `connectors.connection_removed` arrives for the only mailbox of a way in a connect created while the connectors module still holds that connection, or while it cannot answer
- **THEN** the way in, its record and its sessions stay

#### Scenario: A failed hand-over
- **WHEN** a connect adds a way in but mail access was declined at the provider, or the connectors module refuses the grant
- **THEN** the way in it added is gone again and no mailbox record is left

#### Scenario: A record the round trip never named
- **WHEN** a connect's record names no connection because the round trip ended before naming it, and `connectors.connection_added` arrives for it
- **THEN** the record names that connection, and a replay changes nothing

#### Scenario: Someone else's connection names nothing
- **WHEN** `connectors.connection_added` names the same provider account but a different grantor account
- **THEN** no record is named or written

#### Scenario: A connection that does not exist names nothing
- **WHEN** `connectors.connection_added` names the grantor's provider account but a connection the connectors module does not hold
- **THEN** no record is named, and the connection it holds still names the record

#### Scenario: A record nothing ever named
- **WHEN** a record still names no connection a day after it was written, and the connectors module holds no such connection
- **THEN** the prune deletes it and removes the way in it alone kept, with one `identity.way_in_removed`, unless that way in is the account's last

#### Scenario: A connected mailbox whose record was never named
- **WHEN** a record still names no connection a day after it was written, but the connectors module holds the grantor's connection of that account in that workspace
- **THEN** the prune names the record with it, and the way in stays

#### Scenario: A connect retried a day later
- **WHEN** a connect is retried for a record left unnamed by an attempt two days ago
- **THEN** the record is written afresh, and a prune while the retry is in flight leaves it and the way in

#### Scenario: The hand-over breaks
- **WHEN** the hand-over task panics
- **THEN** the browser goes back to where it started with `link_error=internal_error`, not a server error page

#### Scenario: Two of its mailboxes removed at once
- **WHEN** the two workspaces' mailboxes of a way in a connect created are removed concurrently
- **THEN** exactly one of the two removals removes the way in

#### Scenario: Whoever signed in with it is signed out
- **WHEN** a browser is signed in with that way in when its last connection is removed
- **THEN** that session has ended

#### Scenario: Other ways in stay
- **WHEN** the connection of a shared mailbox, of the account the person signed up with, or one never recorded is removed
- **THEN** the person's ways in are unchanged

#### Scenario: The last way in stays
- **WHEN** the only way in left is one a connect created and its connection is removed
- **THEN** it stays
