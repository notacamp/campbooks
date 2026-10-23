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

Missing `Message-ID` (rare: some automated senders): the id is
`sha256(Date, From, To, Subject, size)` prefixed `digest:`; two genuinely
identical header sets collapse, which is acceptable.

| | Change detection |
|---|---|
| Re-list every folder every run | O(folder size) per run |
| **`STATUS` per folder, then CONDSTORE/QRESYNC where offered** (chosen) | one round trip per folder when nothing moved |
| IDLE | needs long-lived sessions; #318 |

## The cursor

Per stream (jsonb), per tracked folder: `name` (as the server spells it,
modified UTF-7 kept raw), `role` (`inbox|sent|junk|trash|archive|user`),
`uidvalidity`, `uidnext`, `highestmodseq` (when CONDSTORE), `messages`,
`backfill_low_uid` (how far down the window the walk has reached), plus
`window_start`, `generation`, `swept_at`. The folder list is refreshed
(`LIST`) at most every 30 minutes or when a `SELECT` fails with
`[NONEXISTENT]`.

## A run

One session per run (the stream is already locked per run), host guard
and TLS as in #310, LOGOUT at the end. A run stops starting downloads at
`CAMPBOOKS_CONNECTORS__IMAP__RUN_BUDGET_BYTES` (64 MiB) or 50 messages
and sets `more`.

1. `STATUS` every tracked folder (`UIDNEXT UIDVALIDITY MESSAGES` +
   `HIGHESTMODSEQ` when offered). Unchanged and not mid-backfill → skip.
2. **UIDVALIDITY changed** → forget that folder's locations, set
   `backfill_low_uid` to none and re-walk the window; messages already
   known by `Message-ID` are re-located, not re-downloaded.
3. **New UIDs** (`uidnext` grew): `UID FETCH old_uidnext:* (UID FLAGS
   RFC822.SIZE INTERNALDATE BODY.PEEK[HEADER.FIELDS (MESSAGE-ID DATE FROM
   TO SUBJECT)])`; known `Message-ID` → new location + state re-emitted;
   unknown → `BODY.PEEK[]` (skipped and recorded as oversized above
   `MAX_MESSAGE_BYTES`, 50 MiB), parsed with the shared `mail::parse`.
4. **Flag changes**: with CONDSTORE `UID FETCH 1:* (FLAGS) (CHANGEDSINCE
   m)`; with QRESYNC also `VANISHED`. Without CONDSTORE, flags and
   expunges are found by the sweep.
5. **Backfill** (phase `backfill`, or a re-walk): `UID SEARCH SINCE
   <window_start>` per folder, newest first below `backfill_low_uid`, as
   in step 3.
6. **Sweep** every `CAMPBOOKS_CONNECTORS__IMAP__SWEEP_MINUTES` (360):
   `UID FETCH <window> (FLAGS)` per folder and `UID SEARCH` of the window;
   a location the server no longer has is dropped. A message with no
   location left is looked for once by `Message-ID` (`UID SEARCH HEADER
   Message-ID`) in the other tracked folders; found → moved; not found →
   removed.

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

Schema `connectors`. Migration in the slot the merge coordinator assigns:
`connectors.imap_locations (stream_id, message_key, folder, uidvalidity,
uid, PRIMARY KEY (stream_id, folder, uidvalidity, uid))` with an index on
`(stream_id, message_key)` and workspace RLS like `connectors.messages`.
`message_key` is the index's provider message id. Events unchanged
(the change stream and labels as for Zoho).

## Health and limits

- A login refused at sync three runs in a row → `needs_reauth`
  (`imap_login_refused`); one refusal counts as a failed run.
- `[UNAVAILABLE]`, `[INUSE]`, `[LIMIT]` or a connection refused/reset →
  throttle that stream for the adaptive interval doubled (no
  `Retry-After` in IMAP).
- `CAMPBOOKS_CONNECTORS__IMAP__SESSIONS_PER_HOST` (default 4) concurrent
  sessions per IMAP host across all processes, through the shared
  `provider_budgets` table; a run that can't get a slot waits for the
  next dispatch.

## Settings

`CAMPBOOKS_CONNECTORS__IMAP__*` gains `BACKFILL_DAYS` (90),
`MAX_MESSAGE_BYTES` (50 MiB), `RUN_BUDGET_BYTES` (64 MiB),
`SWEEP_MINUTES` (360), `SESSIONS_PER_HOST` (4), `MAX_FOLDERS` (200).

## Risks

- **Servers that lie about CONDSTORE** or reset UIDVALIDITY often: the
  sweep and Message-ID identity bound the damage to extra reads.
- **Huge folders**: the window and `MAX_FOLDERS` bound a backfill; the
  sweep only reads flags.
