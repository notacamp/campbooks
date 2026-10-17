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
records the document of a released send and every change request), `connectors`
(asks it), the server (registers it). Two migrations. No new endpoint,
event, scope or dependency.

## What Changes

- **The outbound-requests port** (`kernel::outbound::OutboundRequests`,
  registered with `AppState::with_outbound`; until then nothing stands).
  `stored_send` answers whether a send stands and with which document;
  `changes_standing` answers, item by item, whether a change request was
  made by that module with that item. `conversations` implements it over
  its own tables.
- **A send is re-checked twice before the provider.** `connectors.queue_send`
  records a send only when the port confirms it: the draft is `sending` as
  that send, from the event's inbox, its author is a member who may still
  send it (the same check `release_send` makes), and a document is recorded
  for it. The document recorded is the one the port returns, never the
  event's. `connectors.send_message` asks again before any provider call
  and sends only while the port confirms it with the document recorded. A
  send that no longer stands is never sent: an unclear earlier attempt is
  still looked up (a read, so one that went out is recorded sent), and
  otherwise the send is settled `unavailable` with no provider call.
- **The document is recorded with the send.** `conversations.release_send`
  stores the outgoing document's blob reference on the draft
  (`conversations.drafts.send_document`, migration `20261021200000`) in the
  transaction that marks it `sending`.
- **Change requests are recorded where they are made.** Every
  `changes_requested` the conversations module publishes is recorded in
  `conversations.change_requests` under the event's id (migration
  `20261021200100`). `connectors.request_changes` records only the items the
  port finds in that request, before it asks Gmail for a history stamp. A
  forged event (a fresh id) holds nothing.
- **One provider call per attempt, from the send's own mailbox.**
  `send_message` claims each attempt atomically (pending, and no other
  attempt since it read the send), so two jobs of one send never both call;
  and it uses a stream only of the send's own inbox.
- On a mismatch the delivery completes as a no-op with a WARN line (stable
  message, ids and counts only) and is not retried. A port error retries.

## Non-goals

- **An attacker with SQL in the workspace.** They can skip the event:
  insert a `scheduled` draft of their own with a member as author and plant
  a `conversations.release_send` job, and the mail goes out. What this
  change guarantees is that an event alone is not enough and that the event
  payload is never what gets sent or changed. Binding a release to a key
  outside the database is campbooks-cloud#227.
- Rows written directly into `connectors.outbound_changes` (same reason).
- A change to the event contracts. The events keep their shape;
  `send_requested.message` is still published but no longer trusted.

## Impact

- A transaction working in a workspace can no longer make its mailbox send,
  or change mail at the provider, by appending an event alone, nor choose
  the content sent by naming it in an event.
- A draft released by an older image (no recorded document) during a rolling
  deploy is not sent: it fails `unknown_outcome` after `check_send`'s hour
  and can be sent again. See `design.md`.
