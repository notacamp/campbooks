## Why

The re-review of #560 (campbooks-cloud#233) found that `list_senders` with
`reachable` (Forward's picker, on every keystroke) checked every sender's
mail, spam included, with no index that fits: on a large inbox it walked a
year of mail per request, against the spec's promise that the cost doesn't
grow with an inbox's history (campbooks-cloud#246). Undoing "It's safe" was
also untested.

## What Changes

- Three indexes, each built CONCURRENTLY in a migration of its own:
  `messages (inbox_id, sender_address, sent_at DESC) WHERE direction =
  'inbound'`, `needs (message_id)`, `corrections (message_id, question)`.
- The flag check runs on the ranked senders, best first, only until the
  answer is full (`OFFSET 0` keeps it out of the sort), each sender through
  the new index, instead of over every sender's mail at once.
- A test: after "It's safe" is undone (`withdrawn_at`), the sender is left
  out again.

## Impact

- Spec: conversations (Who writes to the caller).
- Code: `list_senders` (query), three migrations.
- Measured with EXPLAIN ANALYZE on a 312,000-message demo inbox (see the PR).
