# Proposal

## Why

An IMAP mailbox (#311) is read, and pushed with IDLE (#318), but a member's
changes in Campbooks stay in Campbooks: `connectors.request_changes` records
them `skipped` (`unsupported`) for IMAP streams. Marking a conversation
read, starring it, archiving it, putting it in the trash or spam, or moving
it to another stream must reach the mailbox, as it does for Gmail and Zoho,
so the member sees the same mailbox in Campbooks and in any other mail app.

IMAP is not a label store. A message is a copy in a folder, addressed by a
UID that is only valid with its folder's UIDVALIDITY; moving it gives it a
new UID; deleting is two steps (`\Deleted`, then `EXPUNGE`), and a plain
`EXPUNGE` deletes for good every message marked `\Deleted` in the folder,
including ones the member marked in another app. Write-back has to respect
all of that, against servers that may be old, odd or hostile.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/312
(epic https://github.com/notacamp/campbooks-cloud/issues/308).

Modules touched: `connectors` only (the IMAP adapter's write path, its
reader's view of stream folders, and the outbound-change recorder's IMAP
mapping). No new module, no new crate, no new cross-module dependency, no
migration, no new endpoint, scope, event or job.

## What Changes

- **Flags.** Read and unread, starred and unstarred become `\Seen` and
  `\Flagged` on every copy of the message, with `UID STORE +FLAGS.SILENT` /
  `-FLAGS.SILENT`; with CONDSTORE, `UNCHANGEDSINCE` the modseq the index was
  read at, so a change made at the server since wins.
- **Moves.** Archive, unarchive, trash, untrash, spam, unspam and a move to a
  stream move copies between the folders the reader tracks: `UID MOVE`
  (RFC 6851) where offered, else `UID COPY` + `UID STORE +FLAGS.SILENT
  (\Deleted)` + `UID EXPUNGE` of exactly those UIDs (UIDPLUS, RFC 4315). A
  server with neither gets no move (`unsupported_here`): never a plain
  `EXPUNGE`, never `CLOSE`.
- **Streams mirrored as folders.** Primary is the Inbox; Updates, Receipts
  and Newsletters are `Campbooks/<Stream>` folders under the server's
  hierarchy delimiter, created (and subscribed) the first time one is
  needed, their names encoded in modified UTF-7, bounded, and refused when
  they could escape or nest. A mailbox gets at most
  `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` (default 10) folders from
  Campbooks. The reader counts a stream folder as part of the inbox and
  reports moves between stream folders and the Inbox as the message's
  `provider_stream`, so streams sync both ways.
- **No echo.** The write updates the message's locations
  (`connectors.imap_locations`: flags, and the new UID from `COPYUID` when
  the server gives one) and its index state in the transaction that ends
  the change, before the run reads; the read that follows, and the run an
  IDLE session asks for after hearing the write, find nothing new.
- **Only what stands.** Writes act only on the locations this stream
  recorded, in the stream's workspace, after the #226 standing check and the
  #242 binding (unchanged). A folder whose UIDVALIDITY changed is not
  written (the next read re-locates); a UID the server no longer has is a
  no-op.
- **Sessions and limits.** Writes go in the stream's run, over its one
  session (`SELECT` only for folders written; reading stays `EXAMINE` and
  `BODY.PEEK`), so they count against the existing per-host, per-workspace
  and per-login caps and never open a second session beside an IDLE one.
  UIDs go in ranges, at most 500 a command; every answer is bounded by the
  session's framer; a busy server backs off; a change meets trouble at most
  eight times before it is `failed`.
- Still behind `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` (off by default).

## Non-goals

- Label (user folder) changes from Campbooks: `add_label` / `remove_label`
  stay `skipped` (`unsupported`) for IMAP; removing a copy from a folder is
  a deletion and needs its own design.
- Putting a message back in the folder it was trashed from (Zoho's
  `restore_to`): untrash goes to the Inbox for received mail, Sent (or
  Archive) for the member's own.
- Unarchiving into the stream folder the message came from: unarchive goes
  to the Inbox.
- Emptying the Trash, deleting or renaming any folder, keywords (`$Label1`).
- `UTF8=ACCEPT` (RFC 6855): folder names are always sent in modified UTF-7.
- Sending (#313).
