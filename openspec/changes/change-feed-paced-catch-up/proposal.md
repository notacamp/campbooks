# Proposal

## Why

The second security review of the change feed (campbooks-cloud#131, PR
#514) left three notes (campbooks-cloud#198):

- **Catch-up timing is a weak channel.** A catch-up page is cut after 200
  visible changes, but its read scans and skips every hidden change in
  between, at a few microseconds each. The gap between catch-up bursts
  therefore grows with the hidden volume. `design.md` said the pages are
  fixed by the visible changes alone, which holds for how frames group, not
  for when they leave.
- **Load.** Every read lists the caller's inboxes with one more query
  (`visible_containers`), on every wake-up of every stream, outside the
  feed's query permit.
- **Membership re-check.** A stream resolves membership again only when a
  page shows a membership change, so a removed member whose removal it
  does not show keeps receiving ids until the next heartbeat (25 s at most).

Modules touched: the kernel (`changes`, `config`) and `conversations` (an
inbox made for a member is announced). The server changes in tests only. No
new module, no new cross-module dependency, no migration.

## What Changes

- **Catch-up pages leave on a fixed tick.** From the open, and after any
  full page, a stream is behind the head. Each read made while behind
  releases its frames at the first tick after the read ends, ticks counted
  from the open (`CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS`, 50 ms by default,
  50 to 1000, checked at startup). So a page whose read fits in a tick
  leaves exactly on the tick, however many hidden changes it went past. A
  stream that caught up sends what a wake-up brings at once, as before.
- **The containers are listed once and kept per stream.** The list is kept
  until a change that may change who sees inside a container (the
  visibility port's `access_records`: `conversations.inbox`) is numbered in
  a range the stream read, whether or not the caller may see that change,
  and the stream lists them at every heartbeat. When such a change is
  numbered, the stream lists the containers again and reads the page anew
  before showing any of it. The listing runs under the feed's query permit,
  with the page.
- **Membership is re-checked on any membership change.** A change of a
  membership record type (`workspaces.member`, `workspaces.workspace`)
  numbered in a page's range, seen or not, makes the stream resolve
  membership before showing that page. A removed member's stream ends at
  once, even when the removal is not one they may see.
- **A new inbox is announced.** When a `messages` grant adds the member to
  an inbox, `conversations.create_inbox` publishes
  `conversations.inbox_changed` (within the inbox). Open streams of that
  member list their inboxes again before reading on, so the new inbox's
  first mail reaches them live.

## Non-goals

- Reads on a fixed clock for the live tail. A stream that caught up still
  reads when a change is numbered (a hidden one too); that residual is
  unchanged and stays documented.
- Caching `container_visible`, the second look at each container of a page
  before it is shown (defence in depth).

## Impact

- A catch-up delivers at most one page (200 changes) per tick: 4,000
  changes a second at the default tick. Catching up after a long absence
  takes longer, and each stream's catch-up scans the database at most once
  per tick.
- The first frames after an open wait for the first tick (50 ms by
  default).
- A busy workspace's streams list containers about once a heartbeat
  instead of on every wake-up. Each read adds one range query on the
  access record types, which scans the same index range as the page.
