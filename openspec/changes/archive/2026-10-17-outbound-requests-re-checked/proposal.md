# Proposal

## Why

The review of campbooks-cloud#211 (public #541) left one gap, tracked as
campbooks-cloud#226. `connectors.queue_send` and `connectors.request_changes`
act on `conversations.send_requested` and `conversations.changes_requested`
without checking the state those events describe. Any transaction working
in a workspace can append that workspace's events (the defence-in-depth
threat model of #526). A forged `send_requested` could make the workspace's
connected mailbox send mail no member asked for, with content the event
names. A forged `changes_requested` could mark read, archive, star or
relabel mail at the provider.

Modules touched: the kernel (a new port), `conversations` (implements it,
records the document of a released send), `connectors` (asks it), the
server (registers it). One migration. No new endpoint, event, scope or
dependency.

## What Changes

- **The outbound-requests port** (`kernel::outbound::OutboundRequests`,
  registered with `AppState::with_outbound`; until then nothing stands).
  `stored_send` answers whether a send stands and with which document;
  `changes_standing` answers, item by item, whether a requested change is
  what the conversation shows. `conversations` implements it over its own
  tables.
- **A send is re-checked twice before the provider.** `connectors.queue_send`
  records a send only when the port confirms it: the draft is `sending` as
  that send, from the event's inbox, its author is a member who may still
  send it (the same check `release_send` makes), and a document is recorded
  for it. The document recorded is the one the port returns, never the
  event's. `connectors.send_message` asks again before any provider call
  (the lookup of an unclear attempt included) and sends the port's document,
  which must equal the one recorded. A send that no longer stands is settled
  `unavailable` (or `unknown_outcome` after an attempt that may have gone),
  with no provider call.
- **The document is recorded with the send.** `conversations.release_send`
  stores the outgoing document's blob reference on the draft
  (`conversations.drafts.send_document`, migration `20261021190000`) in the
  transaction that marks it `sending`.
- **A change is re-checked before it is recorded.** `connectors.request_changes`
  records only the items the port says the conversation shows (the message
  is read for `mark_read`, the label is on the conversation under that key
  for `add_label`, the conversation is in that stream for `move_to_stream`,
  and so on), before it asks Gmail for a history stamp.
- On a mismatch the delivery completes as a no-op with a WARN line (stable
  message, ids and counts only) and is not retried. A port error retries.

## Non-goals

- Rows written directly into `connectors.outbound_changes`: a transaction
  that can do that can equally write the conversations tables the port
  reads. `connectors.sends` is covered anyway, because `send_message` asks
  the port again before every provider call (see `design.md`).
- A change to the event contracts. The events keep their shape;
  `send_requested.message` is still published but no longer trusted.

## Impact

- A transaction working in a workspace can no longer make its mailbox send,
  or change mail at the provider, by appending an event alone.
- A draft released by an older image (no recorded document) during a rolling
  deploy is not sent: it fails `unknown_outcome` after `check_send`'s hour
  and can be sent again. See `design.md`.
