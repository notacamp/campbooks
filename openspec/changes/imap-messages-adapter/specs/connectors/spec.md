## ADDED Requirements

### Requirement: IMAP messages adapter
The IMAP adapter SHALL read a mailbox's Inbox, Sent, Junk, Trash, Archive
and user folders (never Drafts, `\All`, `\Flagged` or unselectable folders),
first the configured window (default 90 days) newest first, then each run
only the folders whose `STATUS` moved, and the window's flags again every
`CAMPBOOKS_CONNECTORS__IMAP__SWEEP_MINUTES` (default 360). Folder roles SHALL
come from SPECIAL-USE attributes, or from well-known folder names when the
server gives none. New messages SHALL be fetched with `BODY.PEEK[]` as raw MIME
and stored like Gmail's; reading SHALL never change a flag at the server.

#### Scenario: Backfill then incremental
- **WHEN** a new IMAP mailbox's stream runs until its backfill ends and a message then arrives in the Inbox
- **THEN** the window's messages are announced once each, and the next run announces only the new message

#### Scenario: Nothing moved
- **WHEN** a run finds every folder's `UIDNEXT`, `UIDVALIDITY` and message count unchanged and the server has no CONDSTORE
- **THEN** no folder is opened and nothing is announced

#### Scenario: Read in another app
- **WHEN** a message is marked read in another mail app on a server with CONDSTORE
- **THEN** the next run announces it as read without downloading it again

#### Scenario: Reading does not mark read
- **WHEN** the adapter downloads an unread message
- **THEN** the message is still unread at the server

#### Scenario: A server without SPECIAL-USE
- **WHEN** the server lists `INBOX.Sent Items` and `INBOX.Bulk Mail` without attributes
- **THEN** they are read as Sent and Junk

#### Scenario: Oversized message
- **WHEN** a message is larger than `CAMPBOOKS_CONNECTORS__IMAP__MAX_MESSAGE_BYTES`
- **THEN** it is recorded as oversized and not downloaded

### Requirement: An IMAP message is one message wherever it is
An IMAP message SHALL be identified by its normalised `Message-ID` (or a
digest of its headers when it has none), so that moving it between folders,
copying it, or a folder's UIDVALIDITY changing never creates a second
message. Its state SHALL be derived from all its locations, and each location
SHALL be kept for write-back.

#### Scenario: Moved in another app
- **WHEN** a message is moved from the Inbox to a user folder `Bakery` in another mail app
- **THEN** the same message is announced as archived with the label `Bakery`, and no new message appears

#### Scenario: UIDVALIDITY reset
- **WHEN** the server changes the Inbox's UIDVALIDITY
- **THEN** the Inbox is re-read, its messages are re-located by `Message-ID` without being downloaded or announced as new

#### Scenario: Deleted in another app
- **WHEN** a sweep finds a message in no tracked folder
- **THEN** it is announced as removed

### Requirement: IMAP sessions are bounded and failures are classified
A sync run SHALL use one session, through the host guard and TLS of
`imap-connection`, and SHALL hold one of at most
`CAMPBOOKS_CONNECTORS__IMAP__SESSIONS_PER_HOST` (default 4) sessions per host
across all processes. A login refused in three consecutive runs SHALL set
the connection's health to `needs_reauth`; a server answering
`UNAVAILABLE`, `INUSE` or `LIMIT`, or refusing the connection, SHALL only
slow that stream down.

#### Scenario: Password changed at the provider
- **WHEN** the IMAP server refuses the stored password in three consecutive runs
- **THEN** the connection's health becomes `needs_reauth`, its streams pause, and `ConnectionNeedsReauth` is published once

#### Scenario: Busy server
- **WHEN** the server answers `[UNAVAILABLE]` at login
- **THEN** the stream is retried later and its health is not `needs_reauth`

#### Scenario: Many mailboxes on one provider
- **WHEN** five streams on the same IMAP host are due at once
- **THEN** at most four run at the same time and the fifth runs at a later dispatch
