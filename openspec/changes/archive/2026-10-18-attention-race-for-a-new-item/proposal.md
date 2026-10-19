## Why

`replaying_rebuilds_the_items` failed now and then in CI (campbooks-cloud#135):
after a replay, the item was `reply` at revision 1 instead of `do` at revision
2. The cause is in the product, not the test. The projection locks the item
row (`FOR UPDATE`) before deciding whether an event is newer, but when there
is no row yet there is nothing to lock: two deliveries for the same new
conversation, run by two workers at once, both insert. The upsert then
overwrote unconditionally, so whichever landed second won, even when it was
older. The same can happen in production when a conversation's need changes
twice in quick succession before it has an item, leaving Home on an older
need until the next change.

## What Changes

- The item upsert (open and clear) only writes when its revision is newer
  than the stored one (`ON CONFLICT … DO UPDATE … WHERE producer_revision <
  EXCLUDED.producer_revision`); a write that changed nothing announces
  nothing.
- A regression test writes revision 2, then the older open and clear with
  ids of their own (the losing interleaving), and checks nothing changes.

## Impact

- Spec: attention (Projecting conversation needs).
- Code: `attention.project_conversation_need` (runtime queries; their offline
  cache entries removed).
