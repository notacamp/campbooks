## Why

The independent review of #545 (campbooks-cloud#232) found that only the
heartbeat's listing checked for a container gained with no announcing
change. A listing caused by a change about some other container added the
unannounced container without the reset, so mail read past under the old
list was never sent again. Only a bug gains a container unannounced, but
that is exactly what the heartbeat's check is a safety net for.

## What Changes

- Every listing of a stream's containers checks for an unannounced gain:
  a gained container that none of the container changes behind the listing
  named (as its record or its container) makes the stream send `reset`
  (`access_changed`). At a heartbeat, or after a change of access, nothing
  was named, so any gain resets, as at a heartbeat before.
- An announced gain (a grant, a new inbox announced inside itself) still
  needs no reset.
- The catch-up pacing test's slack becomes a fifth of a tick, and a comment
  says when a read begins (once the previous frame is buffered).

## Impact

- Spec: change-feed (Access is asked again on every page).
- Code: `kernel::changes::stream` (`access_changes` returns what the
  container changes named).
