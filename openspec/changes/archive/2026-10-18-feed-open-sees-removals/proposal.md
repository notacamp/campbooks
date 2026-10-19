## Why

The security review of #531 (campbooks-cloud#198) found that a stream
opened without a token starts reading membership changes after the head it
reads. A removal numbered between the request resolving the caller's access
and that head read fell in no read range, so the former member got a stream
until the next heartbeat, up to 25 seconds (campbooks-cloud#216, item 1).

## What Changes

- `changes::open` resolves the caller's access again right after it reads
  the head.
  - A removal numbered up to that head is seen, and the open answers 404 as
    for any non-member (a revoked client too).
  - A changed role is the access the stream starts with.
- Membership changes after the head are already in the stream's read range,
  so the gap closes.
- The issue's other items (a shutdown frame, re-checking a stalled client's
  queued page, an end-to-end grant test, caching the second container look)
  are not small, and stay open in the issue.

## Impact

- Spec: change-feed (Access is asked again on every page).
- Code: `kernel::changes::stream::open`; a server test.
