## Why

The change feed filters every change by what the caller may see, but the
stream still carried the workspace's own change sequence. A member outside
an inbox could learn how much mail it gets, and when, from the stream alone
(campbooks-cloud#131, part of #117):

- `ready` carried `head`, the workspace's newest sequence number;
- `change` frames carried the raw sequence as `id`/`cursor`, so the gap
  between two delivered changes counted the hidden ones in between;
- a `heartbeat` event (instead of a comment) was sent only when the cursor
  had moved past hidden changes, and carried the new number;
- resuming answered `too_far_behind` once hidden changes pushed a cursor
  more than `MAX_BACKLOG` behind, and `cursor_expired` when the first
  change after it (possibly a hidden one) was pruned, so the reset reason
  and its timing tracked hidden activity;
- `unknown_cursor` for a number above the head let anyone find the head by
  trying numbers;
- changes of one record were collapsed per page of 200, and where a page
  ends depends on the hidden changes in it;
- a page was 200 changes of the workspace, hidden ones included, so during
  a catch-up the size of each burst of frames counted the hidden changes in
  that page.

## What changes

- **Pages hold visible changes only**: the page query filters by the
  caller's read scopes, audience and visible containers (a new
  `Visibility::container_types`, listed on every read) before it is cut.
- **Cursors become sealed resume tokens.** Every `ready`, `change`,
  `heartbeat` and `reset` frame carries a new opaque token (AES-256-GCM-SIV
  under a key derived from the instance secret, fresh nonce, fixed length,
  bound to the workspace and the caller) as `cursor` and SSE `id`. It holds
  a position and a time, never shown.
- **`ready` carries no `head`.**
- **Heartbeats are `heartbeat` events on a fixed timer** from the open, busy
  or idle, each with a fresh token; a heartbeat's token resumes past the
  hidden changes read before it.
- **Every visible change is one frame** (no per-page collapsing).
- **A token expires by its age alone**: changes are pruned by when they were
  numbered (new `kernel.changes.sequenced_at`), and a token carries a time
  no later change was numbered before, so a token younger than the
  retention always resumes. `too_far_behind` and
  `CAMPBOOKS_CHANGES__MAX_BACKLOG` are removed; opening streams is rate
  limited per caller instead (`CAMPBOOKS_CHANGES__OPENS_PER_WINDOW`,
  `OPENS_WINDOW_SECONDS`).
- The server warns at startup when no secret key is set (tokens then work
  in one process only).
- A numeric cursor from before this change, a token from another workspace
  or install, or an altered one answers `reset` (`unknown_cursor`).

## Impact

- Kernel: `changes::stream`, `changes::sequence`, `crypto::Signer` (gains
  `seal`/`open`), `ReadRules::readable`, `Visibility::container_types`,
  `ChangesConfig` (`max_backlog` removed, `opens_per_window` and
  `opens_window_seconds` added).
- Conversations: the inbox port names its container type.
- Migration `kernel__change_sequenced_at`.
- Server: the stream's OpenAPI description; tests, including the matrix
  that compares each kind of caller's stream with and without activity it
  may not see.
- Web client: `ready` has no `head`; the client already treats cursors as
  opaque strings. A client holding an old numeric cursor gets one `reset`.
