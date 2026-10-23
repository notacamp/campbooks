# Design

## Context

A member's change reaches a provider in two steps (`provider-stream-labels`,
#226, #242):

1. `connectors.request_changes` (← `conversations.changes_requested`) asks
   the conversations module which items stand, maps each to label tokens
   (`outbound::map` for Gmail, `map_zoho` for Zoho), records them in
   `connectors.outbound_changes` and binds each row's MAC to its stream,
   connection and mailbox. For an IMAP stream it records them `skipped`
   (`unsupported`) today.
2. The stream's next run takes the bound, pending changes
   (`outbound::backlog`), merges them with the shared plan (`domain/push.rs`:
   per message and token the latest wins, earlier ones `superseded`;
   `already_set`; whole or not at all; `retry_or_give_up` after eight runs of
   trouble), makes them through the adapter (`gmail/write.rs`,
   `zoho/write.rs`), records how each ended with the index's new state
   (`outbound::record_outcome`), and only then reads. The index holding what
   was written is what keeps the next read from echoing it.

An IMAP stream's state is not a label list but its **locations**
(`connectors.imap_locations`: folder, UIDVALIDITY, UID, `\Seen`,
`\Flagged`, per copy, #311); the index's tokens are derived from them
(`imap::state::tokens`). A run signs in once, holding the host's session
slots (#311) beside at most one IDLE session per login (#318), and reads
with `EXAMINE` and `BODY.PEEK` only.

This change adds `imap/write.rs`, the IMAP arm of both steps, and teaches the
reader stream folders.

## Decisions

### 1. Where writes run: the stream's run, before its read

| | Where IMAP writes are made |
|---|---|
| A job of their own (`connectors.imap_write`) | a second session per login beside the run's and IDLE's, its own caps, and two writers of one stream to order |
| The IDLE session | it exists only on some mailboxes, and must stay in IDLE to hear changes |
| **The stream's run, over its session, before it reads** (chosen) | what Gmail and Zoho do; one session; one writer per stream (the run's ordering key); the read that follows sees the written state |

The run takes the backlog (`outbound::expire`, then `outbound::backlog`,
which verifies each change's binding, #227/#242), signs in, makes the
writes, **records their outcome in its own transaction** (the changes'
ends, the index tokens, the locations, the cursor), and only then reads
over the same session. Recording before the read is what keeps the read
from taking our own writes for the server's.

Sessions: the run already holds one slot of `imap:<host>:<workspace>` and
`imap:<host>` (leaving room for live IDLE sessions), and a login holds at
most one IDLE session plus its stream's one run (#318). Writes add no
session, so they need no cap of their own and never open a second session
for a login an IDLE session holds. A run with writes still ends within the
run's limit (10 minutes) and its soft deadline.

### 2. Flags

`\Seen` and `\Flagged` are set or cleared on **every** copy of the message
in a tracked folder (read means a copy is `\Seen` and starred a copy
`\Flagged`, #311, so unread and unstar must clear them all):
`UID STORE <set> +FLAGS.SILENT (\Seen)` and the like, at most 500 UIDs a
command, in ranges (`1:40,42`). A message changing both flags the same way
gets one command (`+FLAGS.SILENT (\Seen \Flagged)`).

| | Concurrent change at the server |
|---|---|
| Plain `STORE` | last writer wins, but a change made at the server after the stream last read the folder (another app) is overwritten without being seen |
| **CONDSTORE `UNCHANGEDSINCE <modseq the folder was last read at>`** (chosen, where offered) | the server refuses the UIDs changed since (`[MODIFIED …]`): the server's later change wins, the member's is `superseded` (`changed_at_provider`) and the message is emitted again |
| QRESYNC-style per-message modseqs kept in the index | exact, but a new column; the folder's modseq is what the index reflects anyway |

A UID that passed one conditional store in this push is stored without the
condition after it (its own first write raised its modseq). Without
CONDSTORE (or `NOMODSEQ`), the last writer wins.

### 3. Moves

The member's intents become moves between the folders the reader tracks,
of the copies the stream recorded:

| Intent | Copies moved | To |
|---|---|---|
| archive | those in the Inbox or a stream folder | Archive |
| unarchive | those in Archive; else one copy in a member's folder is **copied** | the Inbox (or the stream folder a move asked for) |
| trash / spam | every copy not already there | Trash / Junk |
| untrash / unspam | those in Trash / Junk | the Inbox for received mail; Sent, else Archive, for the member's own |
| move to a stream | those in the Inbox or a stream folder | that stream's folder (the Inbox for Primary) |

A destination the reader does not track (no Archive folder, say) leaves
that part unmade (`not_movable`); a copy outside the tracked folders is
never moved; Drafts are never touched. A move of a message with no copy in
the inbox (archived) to a stream is `skipped` (`not_movable`).

| | How a copy is moved |
|---|---|
| **`UID MOVE` (RFC 6851)**, where offered (chosen first) | atomic; with UIDPLUS it says the new UIDs (`COPYUID`) |
| **`UID COPY` + `UID STORE +FLAGS.SILENT (\Deleted)` + `UID EXPUNGE <those UIDs>`**, with UIDPLUS (RFC 4315) only (chosen second) | `UID EXPUNGE` removes only the UIDs named, so a message the member marked `\Deleted` in another app survives |
| `COPY` + `\Deleted` + `EXPUNGE` | **never**: a plain `EXPUNGE` (or `CLOSE`) deletes for good every `\Deleted` message of the folder |
| Neither MOVE nor UIDPLUS: copy only, original left | a duplicate that is not a move: archive would leave the Inbox as it was and add a copy in Archive |
| Neither: copy, then mark the original `\Deleted` | the original disappears whenever any client next expunges, and meanwhile shows as deleted in other apps: Campbooks would be deciding a deletion it cannot finish |
| **Neither: no move** (chosen) | the change is `skipped` (`unsupported_here`), the message's state emitted again; flags still work. Servers without both are rare today (Dovecot, Cyrus, Gmail, Outlook, Yahoo, Fastmail and iCloud offer MOVE) |

If `UID COPY` worked but marking or expunging the originals did not, the
originals' `\Deleted` is taken off again (`-FLAGS.SILENT (\Deleted)`), the
copies stay (nothing is lost), and the change is `failed` (`refused`). The
write path never sends `EXPUNGE`, `CLOSE`, `DELETE`, `RENAME` or anything
that empties a folder; it leaves a written folder with `UNSELECT` (RFC
3691) where offered, which expunges nothing.

### 4. Streams mirrored as folders

The #302 rule, native first: Primary's native place is the Inbox. IMAP has
no native place for Updates, Receipts or Newsletters, so each is a folder
named like its stream mark, `Campbooks/Updates`, `Campbooks/Receipts`,
`Campbooks/Newsletters`, the `/` being the server's hierarchy delimiter.

| | A stream at an IMAP server |
|---|---|
| A keyword (`$CampbooksUpdates`) | invisible in most apps, and many servers refuse new keywords (`PERMANENTFLAGS`) |
| A copy in the stream folder, the Inbox copy kept | a label-like mark, but every move between streams would delete a copy |
| **The message moved into the stream folder** (chosen) | what other apps show as sorting; one copy; the reader counts the folder as part of the inbox |

- **Found by name**: a folder whose name, decoded, under its parents
  (`Campbooks.Updates`, `INBOX.Campbooks.Updates`), is a stream mark's
  (`marks::of_name`, case-insensitive) is a stream folder (new role
  `stream`). A member's folder already named so is adopted, as at Zoho.
- **Created on first use**: `LIST "" ""` gives the hierarchy delimiter; the
  name is `Campbooks<d><Stream>`, under `INBOX<d>` when every folder the
  server lists is there (a server whose personal namespace is the Inbox's
  children); `CREATE`, then `SUBSCRIBE` (so other apps show it). An
  `[ALREADYEXISTS]` answer is taken as found. The folder joins the cursor
  at once, so the read tracks it and the locations moved into it are never
  orphans.
- **Names are untrusted**: stream names are a fixed set today, but the
  folder name is built as if a member chose it (custom streams are
  planned): each part is refused when empty, longer than 64 characters, or
  holding a control or direction character, the delimiter, `/`, `*` or `%`
  (`LIST` wildcards), or is `.` or `..`; it is encoded in modified UTF-7
  (RFC 3501 §5.1.3) and the whole name bounded (255 bytes). A refused name
  skips the change (`folder_name_refused`), so a name can never create a
  folder elsewhere or nest deeper. `UTF8=ACCEPT` is never enabled, so
  modified UTF-7 is always right.
- **Bounded**: the cursor keeps the names Campbooks created; a new one past
  `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` (default 10) skips the
  change (`folder_cap`); a member deleting one lets it be created again
  under the same name without counting twice.
- **Never removed**: no folder is ever deleted or renamed.
- **Reading**: a copy in a stream folder counts as in the inbox, and is not
  a label. A received message's upsert carries `provider_stream` when the
  server shows where it is: the stream of its stream folder (several: none);
  `primary` for a new message in the Inbox, or one the server moved from a
  stream folder to the Inbox. Otherwise (archived, unchanged, moved by
  Campbooks) it carries none, which leaves the stored stream as it was. So a
  member sorting mail into `Campbooks/Updates` in another app moves the
  conversation, and a message unarchived to the Inbox by Campbooks does not
  pull its conversation back to Primary the next time its flags change.

### 5. No echo

The write records, in the transaction that ends the change: each copy's new
flags; each moved copy dropped and, with `COPYUID` (UIDPLUS), put at its new
folder, UIDVALIDITY and UID; the index tokens recomputed from the copies as
the reader would derive them (a moved copy whose new UID the server did not
say counts at its new folder); and the cursor (a created folder, and
`recheck` when a new UID is unknown, so the next complete run looks the
message up by `Message-ID`). Then the read:

- finds the destination's new UID: the same key at the same place, the same
  tokens, nothing emitted;
- sees the source folder's message count drop: its known UIDs are checked,
  ours already dropped;
- with CONDSTORE, gets our stores back from `CHANGEDSINCE`: the flags the
  locations already hold, nothing emitted.

An IDLE session on the Inbox hears our `FETCH` or `EXPUNGE` and asks for a
run (debounced as #318); that run reads the same way and emits nothing. One
`STATUS` round per folder is the echo's whole cost.

### 6. Conflicts and what stands

- **Only recorded copies of this stream**: the copies come from
  `imap_locations` of the change's stream, read in its workspace's
  transaction; the change itself passed the #226 standing check and its
  #242 binding (unchanged).
- **UIDVALIDITY first**: `SELECT` answers the folder's UIDVALIDITY; when it
  is not the copies' (the folder was reset), none of its copies is written
  and the change stays pending (counted as trouble) for the read, which
  re-locates the messages; a guessed UID is never acted on.
- **Gone first**: `UID SEARCH UID <set>` tells which UIDs are still there;
  a copy the server no longer has is dropped and the next complete run
  looks the message up, and a change whose copies are all gone is
  `skipped` (`message_gone`). A no-op, never an error loop.
- **Last writer wins**, as in decision 2: in the order the server applied
  them, CONDSTORE telling us of later changes where it is offered.

### 7. Hostile servers

- Every command goes through the session's bounded framer (#311): sizes,
  counts and time are set before a byte is read; `COPYUID` and `MODIFIED`
  sets are read only when they name at most 10,000 UIDs, and a `COPYUID`
  whose sets do not pair up is ignored (the new UIDs are then found by the
  read).
- A server that answers `OK` but does not apply the change: each flag
  write is read back (`UID FETCH … (UID FLAGS)`, one command per write); a
  copy whose flags did not change ends the change `failed` (`refused`) and
  the message is emitted again with the server's state; nothing is retried.
  A move it pretends to make (no `COPYUID`, the message still there) is
  found by the read, which looks the message up (`recheck`) and emits where
  the server has it. A change meets trouble (busy,
  a stale UIDVALIDITY, a cut session) at most `MAX_ATTEMPTS` (8) runs, then
  is `failed` (`provider_errors`).
- `[UNAVAILABLE]`, `[INUSE]`, `[LIMIT]` stop the push: unwritten changes
  stay pending and the stream is throttled as for a busy read. A refused
  sign-in counts with the shared refusals (three: `needs_reauth`).
- Logs carry ids, codes and counts only: never a folder name, a stream
  name, an address or the server's words.

## Data ownership and events

Schema `connectors`. **No migration**: `outbound_changes` (kinds, tokens,
states, reasons, binding) and `imap_locations` already hold everything; the
IMAP stream's cursor (jsonb) gains `created` (the folder names Campbooks
created). New reasons on `outbound_changes.reason` (free text, a stable
code): `unsupported_here`, `folder_name_refused`, `folder_cap`. The reader
now honours `connectors.messages.reemit` for IMAP streams (as Gmail's and
Zoho's do), so a refused, superseded or expired change shows the server's
state again. No event, endpoint, scope, job or schedule is added or changed;
`conversations.changes_requested` is consumed as before.

## Tracked events

None added or changed: no new operation, job, schedule or handler, so no
Coverage row. `connectors.request_changes` and `connectors.sync_stream`
already have theirs.

## Settings

`CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` (default 10, 1 to 100),
checked at startup only while `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is on (a
stray value never stops an instance that does not read IMAP), and tied to no
poll bound.
Write-back is on wherever IMAP is (`CAMPBOOKS_CONNECTORS__IMAP__ENABLED`,
default off).

## Risks

- **A wrong sort becomes visible in other apps**, as at Gmail (accepted for
  #302): new mail Scout files in Newsletters moves to
  `Campbooks/Newsletters`.
- **Unarchive lands in the Inbox**, not the stream folder the message came
  from: the conversation keeps its stream in Campbooks, the server shows it
  in the Inbox until the next stream move.
- **A server that lies about CONDSTORE** (`MODIFIED` for everything) turns
  every flag change into `superseded`; the server's state is shown, nothing
  loops.
- **Personal namespace guess**: a server whose folders are all under
  `INBOX.` gets `INBOX.Campbooks.*`; one with a different personal
  namespace prefix (`NAMESPACE`, RFC 2342) and no folders yet under it may
  refuse the `CREATE` (`refused`), leaving stream moves unmade there. Reading
  `NAMESPACE` is a follow-up if one is seen.
