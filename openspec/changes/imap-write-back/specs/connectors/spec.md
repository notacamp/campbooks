## ADDED Requirements

### Requirement: Outbound changes to IMAP
The module SHALL make members' changes to messages of an IMAP stream, with
the merge, supersede, `already_set`, `in_trash`, retry, expiry and give-up
rules of the Gmail write-back and in arrival order, in the stream's run
before it reads. Read and unread, starred and unstarred SHALL set or clear
`\Seen` and `\Flagged` on every copy of the message in a tracked folder
(`UID STORE … +FLAGS.SILENT` / `-FLAGS.SILENT`). Archive SHALL move the
copies in the Inbox or a stream folder to the Archive folder; unarchive
SHALL move the copies in Archive to the Inbox, or copy one from a member's
folder when none is in Archive; trash and spam SHALL move every other copy
to the Trash or Junk folder; untrash and unspam SHALL move those copies to
the Inbox for received mail, and to Sent (else Archive) for the member's
own. A move SHALL use `UID MOVE` when the server offers MOVE, else
`UID COPY`, `UID STORE +FLAGS.SILENT (\Deleted)` and `UID EXPUNGE` of
exactly those UIDs when it offers UIDPLUS, and SHALL NOT be made at all on
a server with neither (`skipped`, `unsupported_here`). The module SHALL
never send `EXPUNGE` without UIDs, `CLOSE`, `DELETE` or `RENAME`, never move
a copy into or out of a folder the reader does not track, and never empty
the Trash. A destination the reader does not track SHALL leave that part
unmade (`not_movable`). Label changes SHALL be recorded `skipped`
(`unsupported`). UIDs SHALL be sent as ranges, at most 500 a command.

#### Scenario: Read and starred round-trip without echo
- **WHEN** a member marks an IMAP message read and starred, the stream runs, then runs again
- **THEN** the server's copy is `\Seen` and `\Flagged`, the change is `applied`, and neither run announces the message

#### Scenario: Unread and unstarred clear every copy
- **WHEN** a member marks unread and unstars a message with a read, flagged copy in the Inbox and another in a member's folder
- **THEN** both copies lose `\Seen` and `\Flagged`

#### Scenario: Archive with MOVE
- **WHEN** a member archives a message in the Inbox of a server offering MOVE and UIDPLUS
- **THEN** the server receives `UID MOVE` to the Archive folder, the message's location is its new UID there, the change is `applied`, and the next run announces nothing for it

#### Scenario: Archive with UIDPLUS only
- **WHEN** a member archives a message on a server offering UIDPLUS and not MOVE
- **THEN** the server receives `UID COPY`, `UID STORE +FLAGS.SILENT (\Deleted)` and `UID EXPUNGE` of that UID only, and the message is in Archive only

#### Scenario: No MOVE and no UIDPLUS
- **WHEN** a member archives a message on a server offering neither MOVE nor UIDPLUS
- **THEN** the message stays in the Inbox, the change is `skipped` (`unsupported_here`), and the server never receives `EXPUNGE`, `COPY` or `\Deleted`

#### Scenario: Another app's deleted message survives
- **WHEN** a message in the Inbox is marked `\Deleted` in another app and a member archives another message of that Inbox on a server offering UIDPLUS only
- **THEN** the other app's message is still in the Inbox, still `\Deleted`

#### Scenario: Trash
- **WHEN** a member puts a message with copies in the Inbox and a member's folder in the trash
- **THEN** both copies move to the Trash folder, the change is `applied`, and no run announces the message

#### Scenario: Untrash received mail
- **WHEN** a member takes a received message out of the trash
- **THEN** its copy moves from Trash to the Inbox

#### Scenario: Unarchive the member's own reply
- **WHEN** a member unarchives a conversation whose only copy of a message is in Sent
- **THEN** the copy stays in Sent and the change is `skipped` (`not_movable`)

#### Scenario: A label change
- **WHEN** a member adds a label to an IMAP conversation
- **THEN** the change is recorded `skipped` (`unsupported`) and the server receives nothing

### Requirement: Streams as IMAP folders
An IMAP stream SHALL carry Primary in the Inbox and Updates, Receipts and
Newsletters in folders named `Campbooks/Updates`, `Campbooks/Receipts` and
`Campbooks/Newsletters` under the server's hierarchy delimiter (under the
Inbox when every folder of the server is), found by name (case-insensitive,
decoded) or created with `CREATE` and `SUBSCRIBE` by the push that first
needs one. A move to a stream SHALL move the message's copies in the Inbox or
a stream folder to that stream's folder. Each part of a folder name
Campbooks creates SHALL be encoded in modified UTF-7, be at most 64
characters, and be refused (`skipped`, `folder_name_refused`) when it is
empty, `.` or `..`, or holds the delimiter, `/`, `*`, `%`, or a control or
direction character; the whole name SHALL be at most 255 bytes. A mailbox
SHALL get at most `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` (default
10) folders from Campbooks (`skipped`, `folder_cap` past it); no folder SHALL
ever be deleted or renamed. The reader SHALL count a copy in a stream folder
as in the inbox and not as a label, and a received message's upsert SHALL
carry `provider_stream` when the server shows where it is: its stream
folder's stream (none for several), `primary` for a new message in the
Inbox or one moved there from a stream folder at the server; otherwise it
SHALL carry none.

#### Scenario: A stream folder created once and reused
- **WHEN** members move two IMAP conversations to Newsletters in two runs and the mailbox has no `Campbooks/Newsletters`
- **THEN** the server receives one `CREATE` of `Campbooks.Newsletters` and one `SUBSCRIBE`, both messages are moved into it, and no run announces them

#### Scenario: Back to Primary
- **WHEN** a member moves a conversation from Newsletters to Primary
- **THEN** its copy moves from `Campbooks/Newsletters` to the Inbox

#### Scenario: A hostile stream name
- **WHEN** a folder name is built for a stream named with the delimiter, `/`, `*`, `%`, a control character, 65 characters, `..`, or nothing
- **THEN** no name is built and nothing is created; a name with non-ASCII letters is encoded in modified UTF-7 as one part

#### Scenario: The folder creation cap
- **WHEN** Campbooks has created `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` folders in a mailbox and a move needs another
- **THEN** no folder is created and the change is `skipped` (`folder_cap`)

#### Scenario: Sorted in another app
- **WHEN** a member moves a message from the Inbox to `Campbooks/Updates` in another mail app
- **THEN** the next run announces it in the inbox, without the folder as a label, with `provider_stream` `updates`

### Requirement: IMAP writes act only on what stands
An IMAP write SHALL act only on the locations recorded for the change's
stream, read in its workspace's transaction, for changes that passed the
standing check and their binding. Before writing a folder the module SHALL
open it with `SELECT` and compare its UIDVALIDITY with the locations': on a
mismatch it SHALL write none of that folder's copies and keep the change
pending for the next read to re-locate them. A UID the server no longer has
SHALL be a no-op: its location dropped, the message looked up by
`Message-ID` at the next complete run, and a change none of whose copies is
left `skipped` (`message_gone`). Where the server offers CONDSTORE, flag
writes SHALL be conditional on the folder's modseq as last read
(`UNCHANGEDSINCE`): a message changed at the server since SHALL keep the
server's state, the change `superseded` (`changed_at_provider`) and the
message emitted again. After a write, the locations (flags; a moved copy at
the UID `COPYUID` names) and the index SHALL hold what was written, in the
transaction that ends the change and before the run reads, so neither that
read nor a run an IDLE session asks for after hearing the write announces
it. A flag write SHALL be checked by reading the copies' flags back: one
the server answered `OK` to without making it SHALL be `failed`
(`refused`), the message emitted again, and never retried. A change SHALL
meet trouble at most eight times before it is `failed` (`provider_errors`).

#### Scenario: UIDVALIDITY changed before the write
- **WHEN** the Inbox's UIDVALIDITY changes after the last read and a member then marks a message of it read
- **THEN** the server receives no `STORE` in that run, the read re-locates the message, and the next run marks it read at its new UID

#### Scenario: Moved elsewhere first
- **WHEN** a message is moved from the Inbox to a member's folder in another app, and before the next read a member archives it in Campbooks
- **THEN** no move is sent for it, the change is `skipped` (`message_gone`), and the next runs announce the message in the member's folder without error

#### Scenario: Changed at the server since the last read
- **WHEN** on a CONDSTORE server a message is marked read in another app and, before the next read, a member stars it in Campbooks
- **THEN** the server keeps its state, the change is `superseded` (`changed_at_provider`), and the message is announced as the server has it

#### Scenario: An IDLE echo of our own change
- **WHEN** the Inbox is held in IDLE and a member's archive of an Inbox message is written
- **THEN** the IDLE session asks for a run, and that run announces nothing

#### Scenario: Another workspace's changes
- **WHEN** a transaction of another workspace reads `connectors.outbound_changes` and `connectors.imap_locations`, or updates them
- **THEN** it sees none of this workspace's rows and changes none

#### Scenario: A server that does not apply what it accepts
- **WHEN** the server answers `OK` to a member's mark read without marking the message read
- **THEN** the change is `failed` (`refused`), the run announces the message unread as the server has it, and no further `STORE` is sent

#### Scenario: The retry cap
- **WHEN** the server answers every write of a change `[UNAVAILABLE]` for eight runs
- **THEN** the change is `failed` (`provider_errors`) and no further write is attempted

### Requirement: IMAP writes share the run's session
IMAP writes SHALL be made over the stream's run's one session, after it
takes its host's session slots, so they count against the per-host,
per-workspace and per-login caps and never open a session beside the
login's IDLE session. Only a folder being written SHALL be opened with
`SELECT`; reading SHALL stay `EXAMINE` and `BODY.PEEK`. Every answer to a
write SHALL be read within the session's bounds; a busy answer SHALL stop
the push and slow the stream down, the unwritten changes staying pending.

#### Scenario: A write waits for a session slot
- **WHEN** a member marks a message read while the stream's host has no session slot free
- **THEN** no session is opened and no `STORE` sent, the change stays pending, and a later run makes it

#### Scenario: A write beside IDLE
- **WHEN** a member marks a message read while the mailbox's Inbox is held in IDLE
- **THEN** the run opens one session for the write and the read, and the IDLE session keeps its lease

#### Scenario: Busy while writing
- **WHEN** the server answers a write `[UNAVAILABLE]`
- **THEN** the change stays pending, the stream is throttled, and the next run makes it
