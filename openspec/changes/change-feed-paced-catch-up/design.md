## Context

Since #514 a page is up to 200 changes the caller may see, filtered in SQL
before `LIMIT`, so how frames group never depends on hidden changes. Two
things still did: when a catch-up page leaves (its read walks
`changes_workspace_seq_idx` past every hidden change before the 200th
visible one, about 1 to 5 µs a row), and how much a read costs (the
caller's inboxes listed on every read, outside the feed's query permit).
This change paces catch-up pages, keeps the containers per stream, and
re-checks membership on any membership change.

## Decisions

### 1. Catch-up pages on a fixed tick

A stream is **behind** from the open until a read ends short of a full
page, and again after any full page. Each read made while behind releases
its frames at the first tick strictly after the read ended, ticks counted
from the open (`Pacer`). A read that ends inside tick *k* leaves at tick
*k + 1*, and the next read starts only once those frames were written. So
with reads shorter than a tick, page *n* of a catch-up leaves at exactly
*n* ticks from the open: the same for a page that skipped no hidden change
and one that skipped 20,000.

- **Tick:** `CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS`, 50 ms by default, 50 to
  1000, checked at startup like the other change-feed settings. Below
  50 ms an ordinary scan would spill past a tick more often; above a
  second a catch-up becomes slow, and a tick could pass the shortest
  heartbeat (1 s).
- **Every behind read is paced**, even one that releases nothing (all its
  rows refused by the second look) or ends the catch-up: it is one page a
  tick, which also bounds a catch-up's database load to one scan per tick
  per stream.
- **The live tail is not paced.** A read that starts caught up (the
  previous one was not full) releases at once, so a change that wakes a
  caught-up stream keeps today's latency. Pacing it too would hide the
  live read's own timing, but that timing is already the accepted residual
  below (reads happen when any change is numbered).
- **The first read after the open is paced** (the stream does not know yet
  whether it is behind), so the first frames after `ready` wait up to one
  tick.
- **Heartbeats** are still due on their own timer. A heartbeat that falls
  in a catch-up makes its read behind too, so the heartbeat frame leaves on
  the following tick, less than a tick late, on a time set by the timer and
  the tick alone.

Alternatives: sleeping a fixed time *before* each read (the scan time would
still show after it); padding each read to a fixed duration (the same as a
tick, but drifting with the work around it); pacing every read (adds up to
a tick to every live change for no gain while the live read timing stays).

### 2. Containers kept per stream

A stream lists the caller's containers on its first read and keeps them
(`Containers`: the page query's `Sight` and `covers`, the head read just
before the list). Every change up to `covers` was committed before the list
was read, so what it did to access is in the list.

The list is dropped, and the next read lists again:

- **when a container-access change is numbered in a range the stream
  read**: after each page query, in the same transaction, one more query
  over the same range (`access_changes`) asks whether a change of a
  `Visibility::access_records` type (`conversations.inbox`) was numbered
  after `covers`. If so, the page is not shown: the stream lists again and
  reads the same range anew. This looks at hidden changes too, on purpose:
  an inbox made for someone is announced within that inbox, which their
  old list does not hold, so only a hidden-inclusive look can see it.
- **at every heartbeat**, so a change of access that nothing announced is
  kept no longer than a heartbeat interval (25 s by default), the bound the
  membership re-check already had;
- **when membership resolves to a different access** (a role change).

Every way of gaining sight inside an inbox now publishes
`conversations.inbox_changed`: the existing ones (a membership ended, an
owner left, an inbox kept read-only) and a new one, `create_inbox` adding
the granting member. Without that, a stream would have read past the new
inbox's first mail with the old list and never shown it. Losing sight was
already safe with a stale list: the second look (`container_visible` for
every container of a page, before showing it) refuses the rows and sends
`reset` (`access_changed`).

**Worst case staleness.** For an announced change: none; the page that
covers the announcement is read again under a fresh list before any of it
is shown, and later changes come after it. For an unannounced one: one
heartbeat interval. A lost container is never shown meanwhile (the second
look), a gained one waits at most that long, and its changes numbered in
between are skipped (as they were before #514 for a container gained
between a read's list and its page).

**Under the query permit.** The listing (the head and the port's query)
runs under the same permit as the page, one permit per read. That is safe:
a permit is never taken inside another, and the port does not take one. It
is cheap: with the cache the listing runs about once a heartbeat, so it
rarely lengthens a permit; when a wake-up of thousands of streams follows
an access change, the listings queue at the permit with their pages instead
of taking the pool.

### 3. Membership re-checked on any membership change

The same query asks whether a change of a membership record type
(`Membership::access_records`: `workspaces.member`, `workspaces.workspace`)
was numbered in the range, after the last re-check. If so, the stream
resolves membership (and whether the token still stands) before showing
the page, seen or not. Before, only a membership change the page showed
did that; a caller who may not read `workspaces.member`, or a removal
announced inside a container they cannot see, waited for the next
heartbeat. Membership changes are rare (joins, leaves, role changes,
renames), so the re-check costs a few queries now and then.

## What is left (accepted)

- **The live tail's read timing** (unchanged from #514). A caught-up
  stream reads when any change of the workspace is numbered (a hidden one
  too) or on the poll. Two visible changes numbered in different runs may
  leave in one burst or two depending on when the stream read, and a
  visible change can wait (50 to 250 ms) behind producers in flight before
  it is numbered. Millisecond timing of live delivery, not counts.
- **A catch-up read longer than a tick.** Pacing hides scan time only while
  a read fits in a tick. A read that scans past enough hidden changes to
  take longer (at 1 to 5 µs a row, some 10,000 to 50,000 hidden changes
  between two visible ones at the default tick) leaves on a later tick, so
  the delay still shows, rounded up to whole ticks. Its cause is noisy
  (cache, load) and the open rate limit slows repeating it, but it is
  repeatable; a longer tick narrows it, at the price of a slower catch-up.
- **When a catch-up ends.** The stream stops pacing after the first read
  that is not full; that depends on visible changes only. Frames a live
  read sends right after it leave at once.
- **Extra reads on access changes.** A change of who sees inside some
  inbox, or of membership, numbered in a range a stream read makes that
  stream list again (or re-check membership) and read the page anew, even
  when the change is not theirs to see. It costs one listing and one page
  query: what every read cost before this change. It can delay the frames
  of that read by milliseconds; in a catch-up, the tick absorbs it.
- **Access-loss timing** (unchanged): a `reset` for a container just lost
  comes at the next read or heartbeat.
- **Unannounced access changes** wait up to a heartbeat (section 2).
- **`version`**, a full page at heartbeat time and resume cost: as #514
  describes.

## Testing

- Kernel, on a paused clock (`tokio::time::pause`): a catch-up of three
  pages with a few hidden changes and with up to 23,000 per page (46 ms of
  scanning) leaves at 50, 100 and 150 ms both times, and the live read
  after it at once; a 72 ms scan leaves a tick later; a live full page
  makes the next read paced again. Config: the tick is refused outside 50
  to 1000.
- Server: a 450-change catch-up past more hidden changes than a page leaves
  its pages at one, two and three ticks (250 ms) from the open in the busy
  and in the quiet workspace, with the same frames. The containers are
  listed once across mail and five wake-ups, listed again when the member
  is taken out of an inbox (`reset`, no id of its new mail) and when an
  inbox is made for her (its first mail arrives at once). A removal the
  member cannot see ends her stream (`revoked`) well before the 25 s
  heartbeat. `create_inbox` publishes `inbox_changed` once, not on replay.
