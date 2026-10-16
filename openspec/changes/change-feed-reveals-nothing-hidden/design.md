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

## Decisions

### 1. Sealed, per-frame resume tokens (option 1), not per-audience sequences

A token seals `Position { seq, at }` (16 bytes) with AES-256-GCM under a key
derived from the instance secret (`Signer::seal`, purpose
`campbooks kernel sealed values v1`), associated data
`changes.cursor v1:<workspace>`. A fresh random nonce every time: two tokens
of one position are unrelated, so neither a token's value nor whether it
changed says anything. Fixed length (60 characters, URL-safe base64), so
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

`too_far_behind` goes: a live stream reads every change of its workspace
anyway, so catching up from a token costs the same reads the stream would
have made; the retention bounds it. `MAX_BACKLOG` is no longer read
(unknown settings are ignored, so a deployment that sets it still starts).

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

- **Delivery latency.** A visible change reaches the client when the stream
  reads it: a few milliseconds later if the stream was just reading hidden
  changes, and its numbering can wait (50 to 250 ms) behind producers in
  flight in the workspace. Closing this would need constant-time reading.
- **Access-loss timing.** When a caller loses sight of a container they were
  shown, the `reset` comes at the next page that touches inbox records or
  the next heartbeat, whichever is first; such a page may be hidden. It
  concerns a container the caller just lost, at most a heartbeat early.
- **A flood at heartbeat time.** If the heartbeat's read is a full page, its
  token keeps the previous position, which expires earlier. It takes more
  than a page of changes numbered within the heartbeat's read.
- **Resume cost.** Resuming reads the hidden changes since the token (at most
  a heartbeat's worth from a heartbeat's token).

## Testing

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
