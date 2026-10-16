## Context

A stream reads every change of its workspace in sequence order and shows
the caller only those they may see (scopes, container, audience). The
sequence is per workspace and dense, so any number derived from it that
reaches the client counts changes the client may not see. The goal: what a
caller receives (every event, id, data field and the heartbeat cadence) is
the same whether or not activity they may not see happens, apart from what
they do see.

## Leak paths found

| Path | How it leaked | Closed by |
| --- | --- | --- |
| `ready.head` | the workspace's newest number | removed |
| `ready`/`reset` `cursor` | the head | sealed token |
| `change` `id`/`cursor` | the raw number: gaps count hidden changes | sealed token, new each frame |
| `heartbeat` event vs `: heartbeat` comment | the event only when hidden changes were passed, with the new number | always an event, fixed timer, fresh token |
| Heartbeat timing | the timer restarted after each delivered page | fixed timer from the open |
| `too_far_behind` | head minus cursor, hidden changes included, against `MAX_BACKLOG` | removed |
| `cursor_expired` | the first change after the cursor, possibly hidden, pruned by its publish time | expiry by the token's age |
| `unknown_cursor` | a number above the head resets: probe the head by trying numbers | tokens cannot be made up |
| Per-page collapsing | a record's two changes in one page of 200 became one frame; hidden changes move page boundaries | one frame per visible change |
| Page boundaries | a page was 200 changes of the workspace, hidden ones included, and its visible frames left as one burst: during a catch-up, a burst of k frames meant 200 − k hidden changes, and gaps between bursts counted pages with nothing visible | pages filtered in SQL: up to 200 changes the caller may see |

## Decisions

### 0. Pages of visible changes, filtered in the database

The page query filters by the caller's sight before `LIMIT`
(`stream::read_visible`): the record types a read operation they hold a scope
for returns (`ReadRules::readable`), the audience predicate (the SQL form of
`Audience::admits`), and `container_id IS NULL OR (container_type,
container_id) IN (<their containers>)`. The containers come from a new
visibility-port method, `Visibility::container_types`, listed with the
existing `visible_containers` on every read, so an access change applies to
the next read. A page is therefore up to 200 changes the caller may see, and
how frames group into bursts depends on visible changes only.

Each read first takes the head and the time (`SELECT head, now()`), then
asks for visible changes in `(cursor, head]`. When the page is not full,
every visible change up to that head was read and the cursor moves to the
head, so the hidden changes in between are tested once and never again. The
query walks `changes_workspace_seq_idx` (`workspace_id, seq`) as a range
scan and filters each row (checked with `EXPLAIN ANALYZE` on 200,000 rows: an
index scan stopping at the 200th visible row, the container list a hashed
subplan); no new index is needed.

Defence in depth: rows the query returns are checked again in Rust (read
rules, audience) and their containers asked once more through
`container_visible` (one may have gone out of sight between the list and
the read); a refused row is not shown. A page that touches membership
records still makes the stream resolve membership before showing it, and
reads the page again if the caller's access changed.

The heartbeat's token moves to the head and time of the heartbeat's own
read, when that read was not full.

### 1. Sealed, per-frame resume tokens (option 1), not per-audience sequences

A token seals `Position { seq, at }` (16 bytes) with AES-256-GCM-SIV under a
key derived from the instance secret (`Signer::seal`, purpose
`campbooks kernel sealed values v1`), associated data
`changes.cursor v1:<workspace>\0<caller>` (`user:<subject>` or
`client:<client id>`): a token opens only for the caller it was given to,
in the workspace it was given in; anyone else's gets `unknown_cursor`. A
fresh random nonce every time: two tokens of one position are unrelated, so
neither a token's value nor whether it changed says anything. GCM-SIV is
misuse-resistant, so a repeated 96-bit nonce (unlikely, but many tokens are
sealed under one key) would only show that two tokens hold the same
position, never the key stream. Fixed length (60 characters, URL-safe base64), so
the length does not grow with the number. A MAC alone (like search's
cursors) would leave the number readable.

Per-audience sequences (option 4) would need an index per member (or per
distinct visibility class) maintained on every change, recomputed whenever
someone joins or leaves an inbox, for the same result. Rejected.

### 2. What a position holds, and expiry by age alone

`seq` is where the next read starts; `at` is a time (database clock) before
which no change after `seq` was numbered, chosen from what the caller sees:

- stream opened or reset: the time of the read that found the head;
- a delivered change: when that change was published (its publisher's
  transaction start, which its `version`, a UUIDv7 event id, already shows
  roughly); later changes were numbered after it was;
- a heartbeat: the heartbeat's time, taken before a read; the position moves
  to what that read reached only if it reached the end of the feed (a full
  page may leave earlier-numbered changes behind; then the previous
  position is sealed again);
- `reset` (`access_changed`): the position already handed out.

Changes are now pruned by when they were numbered (`sequenced_at`, the
sequencing statement's start; a later sequencing can only start after the
earlier one committed, so it is monotonic), instead of when they were
published. A token resumes iff `now - at < retention - 5 minutes`: every
change after it is then still kept, whatever happened in between. The five
minutes cover a numbering whose timestamp precedes its commit (and clock
granularity). The old completeness check stays as a safety net, also
answering `cursor_expired`, so a numbering that stalled longer than that
never makes a client skip a change.

`too_far_behind` goes, and `MAX_BACKLOG` is no longer read (unknown
settings are ignored, so a deployment that sets it still starts). Catching
up from a token reads up to the retention's worth of changes, each tested
once. To keep a client reconnecting in a loop with an old token from
re-scanning that over and over, opening a stream is rate limited per caller
across processes (`RateLimits`, `CAMPBOOKS_CHANGES__OPENS_PER_WINDOW` 30 per
`OPENS_WINDOW_SECONDS` 60 by default, 0 turns it off); the count is of opens
only, so a refusal never depends on hidden activity. A stream reads its
next page only when the previous one was written, so a catch-up for a
client that went away stops at the next write (the response body is
dropped), after at most one more page.

### 3. Heartbeats on a fixed timer

Every `HEARTBEAT_SECONDS` from the open, busy or idle, a `heartbeat` event
with a fresh token. It still re-checks membership and the containers shown.
Firing while busy also replaces the old "resolve again if a heartbeat's
time passed" rule in the page reader.

### 4. No per-page collapsing

Each visible change is its own frame. The client already batches cache
invalidations (50 ms), so the extra frames of a burst cost bytes, not
refetches.

### 5. Clients

The web client stores the last `cursor` as a string and sends it back
(`EventSource`'s `Last-Event-ID`, or `?after=`); it never parsed or compared
it. `ready` loses `head`, which it never used. A client holding a numeric
cursor from before gets one `reset` (`unknown_cursor`) and goes on with a
token. Without `CAMPBOOKS_SECRET_KEY` (development), tokens open only in the
process that sealed them, like search cursors: another process answers
`reset`.

## What is left (accepted)

- **Read timing.** A stream reads when any change of the workspace is
  numbered (a hidden one too) or on the poll. Live, two visible changes
  numbered in different runs may leave in one burst or two depending on when
  the stream read, and a visible change can wait (50 to 250 ms) behind
  producers in flight in the workspace before it is numbered. That is
  millisecond timing of live delivery, not counts; closing it would need
  reads on a fixed clock only. In a catch-up the pages are fixed by the
  visible changes alone.
- **Access-loss timing.** When a caller loses sight of a container they were
  shown, the `reset` comes at the next read (which a hidden change can
  trigger) or the next heartbeat. It concerns a container the caller just
  lost, at most a heartbeat early.
- **A full page at heartbeat time.** If the heartbeat's read is full (more
  than a page of changes the caller may see are waiting), its token keeps
  the previous position, which expires earlier; that depends on visible
  changes only.
- **Resume cost.** Resuming tests the changes since the token once each (at
  most a heartbeat's worth beyond a heartbeat's token), bounded by the
  retention and the open rate limit.
- **`version` (predates this change).** A change frame's `version` is the
  event id, a UUIDv7 whose random bits include a per-thread counter
  (`uuid`'s `now_v7`). Two visible versions from one thread can hint at how
  many UUIDs that thread generated in between, which may include events the
  caller cannot see. It says nothing about which workspace or inbox they
  were for; replacing `version` is out of scope here.

## Testing

Pages: a kernel test pages through the same 450 visible changes with and
without 500 + up to 9-per-change hidden ones (another type, another
container, someone else's audience) and asserts identical pages of 200, 200
and 50. A server test catches up from a token past the same mix in an inbox
the member is not in: the same frames, all before the first heartbeat.

The matrix (`changes::tests`, `assert_blind`): two workspaces set up alike
(owner, member, admin, a member with an inbox of their own; a shared inbox),
every observer following both: Bea (member), Cid (admin and inbox manager),
a client acting for Bea, and a client acting for nobody. Both get the same
visible rounds; one also gets activity the observers may not see: mail into
the other member's inbox, their draft (`user` audience), their connection
(`user_and_managers`), a client's draft (`clients`). For each observer the
two streams must give the same frames in the same order with the same
fields once run-specific values (record ids, event ids, tokens) are replaced
by where they first appear; every token is fresh and 60 characters;
heartbeats carry only a token and arrive on the fixed timer; and resuming
from the first, a middle and the last token of each replays the same.
Re-adding the raw sequence to change frames makes the matrix fail.
