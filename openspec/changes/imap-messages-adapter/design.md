# Design

## Context

`sync_stream::run` loads a stream's `Target`, takes its credential, and
dispatches on `Provider` to an adapter that returns `Option<RunPage>`
(`changes`, `index` ops, `labels`, `cursor`, `phase`, `more`); a
provider-neutral `apply()` stores and announces. #310 added an `imap` arm
that returns `None`. This change implements it in `connectors/src/imap/`
(`read.rs`, `folders.rs`, `state.rs`), next to `gmail/` and `zoho/`.

## Alternatives considered

| | Message identity |
|---|---|
| `folder:uidvalidity:uid` | a move or a UIDVALIDITY reset makes a new message: duplicates in conversations, lost state |
| **normalised `Message-ID`** (chosen) | survives moves, copies and resets; what Rails' IMAP adapter learned to use |
| server `OBJECTID`/`EMAILID` (RFC 8474) | ideal, but few servers offer it; used as a tie-break when present |

The index's id is `mid:` and the SHA-256 (hex) of the normalised
`Message-ID` (what is inside its angle brackets, without white space), so
ids stay bounded and never carry an address; the `Message-ID` itself is the
index's `rfc_message_id`. Missing `Message-ID` (rare: some automated
senders): the id is `sha256(Date, From, To, Subject, size)` prefixed
`digest:`; two genuinely identical header sets collapse, which is
acceptable. With OBJECTID, a copy whose `EMAILID` differs from the ones the
key already has is another message: `<key>~<16 hex of sha256(EMAILID)>`.
Every copy also carries a **fingerprint** (SHA-256 of its `Date`, `From`
and size, from the header fetch): a copy whose fingerprint none of the
key's current copies has is another message reusing the `Message-ID` (a
forgery, an auto-reply, a mailing list's copy of the member's own mail),
keyed `<key>~<16 hex of the fingerprint>` and downloaded on its own. A
message with no copy left (moved) keeps its key whatever its fingerprint.

| | Change detection |
|---|---|
| Re-list every folder every run | O(folder size) per run |
| **`STATUS` per folder, then CONDSTORE/QRESYNC where offered** (chosen) | one round trip per folder when nothing moved |
| IDLE | needs long-lived sessions; #318 |

## The cursor

Per stream (jsonb), per tracked folder: `name` (as the server spells it,
modified UTF-7 kept raw), `display` (decoded, `Projects/Bakery`), `role`
(`inbox|sent|junk|trash|archive|user`), `uidvalidity`, `uidnext`,
`highestmodseq` (when CONDSTORE), `messages`, `backfill_low_uid` (how far
down the window the walk has reached) and `backfilled`, plus `listed_at`,
`generation`, `swept_at`, `login_refusals` and `recheck` (messages left with
no location still to look for). The window slides with each run's time
(`now - BACKFILL_DAYS`). The folder list is refreshed (`LIST`) at most every
30 minutes, or at the next run when a `STATUS` or `EXAMINE` was refused; a
folder is forgotten only when `LIST` no longer shows it, and a listing with
no Inbox is not believed.

## A run

One session per run (the stream is already locked per run), host guard
and TLS as in #310, LOGOUT at the end. Folders are only ever opened with
`EXAMINE` (read-only). A run stops starting downloads at
`CAMPBOOKS_CONNECTORS__IMAP__RUN_BUDGET_BYTES` (64 MiB, after one download
at least) or 50 messages, reads at most 2,000 UIDs' headers (100 a
command), starts nothing new after 2 minutes, and sets `more`; a run takes
10 minutes at most in all.

The IMAP client is Campbooks' own (`imap/session.rs`, `imap/syntax.rs`):
`async-imap` (#310's client) is removed, because it buffers a response
whole, sized by the server's own literal announcement, up to 512 MiB (at
sign-in too), and traces every command it sends, passwords included.
Signing in is `AUTHENTICATE PLAIN` (its response in the command with
SASL-IR, else after the server's go-ahead), else `LOGIN` with quoted
strings, or literals when a quoted string cannot carry the username or
password; the STARTTLS and `LOGINDISABLED` rules of #310 are unchanged, a
busy answer is told from a refusal, and every answer before the sign-in
completes is at most 16 KiB. The connect check (#310's `verify`) signs in
the same way. The server's log filter keeps `async_imap` and `imap_proto`
off whatever `RUST_LOG` says.

Each command has bounds: one response's size (a literal's size is checked
before a byte of it is read), the number of untagged responses, all its
bytes, and its time (from `CAMPBOOKS_CONNECTORS__IMAP__TIMEOUT_SECONDS`,
also per socket read). A response is read into at most 100,000 values,
lists of at most 10,000 items nested at most 8 deep; a `SEARCH` into at
most 1,000,000 UIDs a command; vanished UIDs are matched against the
folder's known copies as they arrive (only those are kept), at most
1,000,000 ranges a command.
A command cut short leaves the session unusable: it is dropped, not logged
out. A run cut short after it read something keeps what it read (applied
at the normal interval); one that read nothing is a failed run.

1. `STATUS` every tracked folder (`UIDNEXT UIDVALIDITY MESSAGES` +
   `HIGHESTMODSEQ` when offered). Unchanged and not mid-backfill → skip.
   A folder that will not open (`STATUS` or `EXAMINE` refused, no
   UIDVALIDITY or UIDNEXT) is **set aside** for an hour (`skipped_until`):
   it counts as read for the backfill, and the folders are listed again at
   the next run. Only a run that read something runs again at once
   (`more`), so a folder that will not open never makes a stream sign in
   back to back.
2. **UIDVALIDITY changed** → forget that folder's locations, set
   `backfill_low_uid` to none and re-walk the window; messages already
   known by `Message-ID` are re-located, not re-downloaded.
3. **New UIDs** (`uidnext` grew): `UID FETCH old_uidnext:* (UID FLAGS
   RFC822.SIZE INTERNALDATE BODY.PEEK[HEADER.FIELDS (MESSAGE-ID DATE FROM
   TO SUBJECT)])`; known `Message-ID` → new location + state re-emitted;
   unknown → `BODY.PEEK[]` (skipped and recorded as oversized above
   `MAX_MESSAGE_BYTES`, 50 MiB), parsed with the shared `mail::parse`. A
   download that times out ends the run and is counted
   (`connectors.skipped_messages`); after three, the message is recorded
   as oversized and never asked for again.
4. **Flag changes**: with CONDSTORE `UID FETCH 1:* (FLAGS) (CHANGEDSINCE
   m)`; with QRESYNC also `VANISHED`. Without CONDSTORE, flags are found by
   the sweep. Without QRESYNC, a message count that does not add up (the
   last count plus the new UIDs) checks the folder's known UIDs
   (`UID SEARCH UID low:high`; an empty answer for a folder that has
   messages is not believed).
5. **Backfill** (phase `backfill`, or a re-walk): `UID SEARCH SINCE
   <window_start>` per folder, newest first below `backfill_low_uid`, as
   in step 3.
6. **Sweep** every `CAMPBOOKS_CONNECTORS__IMAP__SWEEP_MINUTES` (360,
   counted from the end of the backfill): per folder, its known UIDs
   checked (`UID SEARCH`) and their flags read again (`UID FETCH low:high
   (FLAGS)`); a location the server no longer has is dropped.
7. **Messages with no location left**, at the end of any run that read
   every folder (and, after a sweep or when some were left over, those the
   index has with no location): looked for by `Message-ID` (`UID SEARCH
   HEADER Message-ID`, ten to a search, `OR`ed) in every tracked folder,
   at most 50 a run; found → located (moved); not found anywhere →
   removed. One without a `Message-ID` cannot be looked for: removed when
   it is within the window (the walk would have found it), kept otherwise.
   One whose `Message-ID` no search can carry (not ASCII, or longer than
   2,048 bytes) is kept.

## State mapping

From the union of a message's locations:

- `in_inbox`: a location in the Inbox; `in_spam`: in Junk and nowhere
  else; `in_trash`: in Trash and nowhere else;
- `read`: `\Seen` on any location; `starred`: `\Flagged` on any;
- direction outbound when a location is in Sent (or the From address is
  the mailbox's own address and no location is in the Inbox);
- `labels`: the user folders it is in, by their decoded display path
  (`Projects/Bakery`), published through the existing labels table;
- `categories` empty, `provider_stream` untouched, `thread_key` none
  (threading by `In-Reply-To` / `References`, as for Zoho).

## Folder roles without SPECIAL-USE

Exact, case-insensitive names after decoding modified UTF-7, with or
without an `INBOX.` / `INBOX/` prefix: Sent (`Sent`, `Sent Items`, `Sent
Messages`, `Sent Mail`, `Enviados`, `Itens Enviados`, `Envoyés`,
`Éléments envoyés`, `Gesendet`, `Gesendete Elemente`), Junk (`Junk`,
`Spam`, `Bulk`, `Bulk Mail`, `Lixo Eletrónico`, `Correo no deseado`,
`Courrier indésirable`, `Indésirables`), Trash (`Trash`, `Deleted Items`,
`Deleted Messages`, `Bin`, `Lixo`, `Reciclagem`, `Papelera`, `Corbeille`,
`Papierkorb`), Archive (`Archive`, `Archives`, `Arquivo`, `Archivo`,
`Archiv`), Drafts (never read: `Drafts`, `Rascunhos`, `Borradores`,
`Brouillons`, `Entwürfe`). Matching is on folder **names** given by the
server, never on message content (ADR 0018).

## Data ownership and events

Schema `connectors`. Migration `20261023170000_connectors__imap_locations`:
`connectors.imap_locations (workspace_id, stream_id, folder, uidvalidity,
uid, message_key, seen, flagged, email_id, PRIMARY KEY (stream_id, folder,
uidvalidity, uid))` with an index on `(stream_id, message_key)`, rows
removed with their stream, and workspace RLS (enabled, forced,
`workspace_isolation`) like `connectors.messages`. `message_key` is the
index's provider message id; `seen` and `flagged` are the copy's own flags
(the message's state is the union); `email_id` is the server's RFC 8474
id when given. Locations are written in the run's transaction, with the
index. Events unchanged (the change stream as for Zoho; the member's
folders travel as each message's labels).

## Health and limits

- A login refused at sync three runs in a row (counted in the cursor's
  `login_refusals`) → `needs_reauth` (`imap_login_refused`); one refusal
  counts as a failed run.
- `[UNAVAILABLE]`, `[INUSE]` or `[LIMIT]` (at sign-in or on a command) →
  throttle that stream for the adaptive interval doubled (no `Retry-After`
  in IMAP; health `throttled`). At connect, the same answer is
  `imap_busy` (a new message in every locale), not a refused password, and
  does not count towards the failed sign-ins.
- A server that cannot be reached (refused, reset, timed out), a bad
  certificate, or a server that misbehaves (too large, too many responses,
  unreadable) → a failed run: backoff doubling from the poll minimum, and
  `failing` after `CAMPBOOKS_CONNECTORS__FAILING_AFTER` in a row. (A
  connection refused or reset slows the stream down this way rather than
  through `throttled`, so a server gone for good ends up `failing`.)
- `CAMPBOOKS_CONNECTORS__IMAP__SESSIONS_PER_HOST` (default 4) concurrent
  sessions per IMAP host across all processes, of which one workspace
  holds at most `…__WORKSPACE_SESSIONS_PER_HOST` (default 2), through the
  shared `provider_budgets` table: `imap:<host>:<workspace>` then
  `imap:<host>`, each a bucket holding that many slots, one taken per run
  and given back at its end (the workspace's is given back if the host's
  is full); a run that can't get both waits for the next dispatch. The
  buckets refill over 15 minutes, so a slot a dead process held comes
  back; the cap is therefore **approximate**: a run longer than a quarter
  of that (at most 10 minutes) lets one more session start. A lease table
  would make it exact, and is not needed yet.

## Settings

`CAMPBOOKS_CONNECTORS__IMAP__*` gains `BACKFILL_DAYS` (90),
`MAX_MESSAGE_BYTES` (50 MiB), `RUN_BUDGET_BYTES` (64 MiB),
`SWEEP_MINUTES` (360), `SESSIONS_PER_HOST` (4),
`WORKSPACE_SESSIONS_PER_HOST` (2, at most `SESSIONS_PER_HOST`),
`MAX_FOLDERS` (200).

## Risks

- **Servers that lie about CONDSTORE** or reset UIDVALIDITY often: the
  sweep and Message-ID identity bound the damage to extra reads.
- **Huge folders**: the window and `MAX_FOLDERS` bound a backfill; the
  sweep only reads flags.
- **A reused `Message-ID`**: identity by `Message-ID` would let anyone who
  knows the id of the member's mail (it travels in every reply) send a
  message the adapter takes for it. The fingerprint keeps such a message
  apart; a forger who also copies `Date`, `From` and the exact size is not
  told apart (the EMAILID tie-break still is, on servers that offer it).
  A server reporting another size for the same message in two folders
  would show it twice; none is known to.
