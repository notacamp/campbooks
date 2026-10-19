# Design

- Data: `conversations.decisions` (answers) is updated in place; no new table.
  `Facts::copied_only` reads `Envelope::recipient.addresses`, `to` and `cc`
  only: addresses, never text.
- AI: role `decide`. Without the role nothing changes (no model pass). The
  user can undo a need with Not needed, as before.
- Re-deciding keys on the `checked:` stamp: both the backfill's queue and the
  pass's "already answered" check count only answers whose `model_version`
  starts with `checked:`. Prefixing the old stamp with `superseded:` makes
  the message unanswered for both, only for mail where the inbox is in Cc and
  not in To, which keeps the model cost to that mail within the existing
  `decide_backfill_days` window.
- No endpoint, no event.
