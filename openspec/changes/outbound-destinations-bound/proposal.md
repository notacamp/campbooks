# Proposal

## Why

Items 4 and 5 of campbooks-cloud#242 (from the review of public #552). An
attacker with SQL as the runtime role inside workspace W, but not the
instance secret, can no longer make a mailbox send or change mail on its own:
sends and outbound changes carry MACs under a key the database never holds.
But those MACs do not say *where* the send or the change goes.

- **(4)** `queue_send`, `send_message` and `request_changes` resolve
  inbox → stream → connection (and the stream's mailbox at the provider)
  from `connectors.streams` when they act. SQL can repoint a stream (its
  connection, its mailbox key, which inbox it claims, or the stream a send
  row names) after the member asked, so a send or change the member really
  made goes out through another connected mailbox in W.
- **(5)** Zoho's `restore_to` (where a trashed message goes back to on
  untrash) is not covered by any MAC, so SQL can choose where a restored
  message lands.

After this change a send or a change goes out only through the connection,
stream and mailbox it was bound to; a repointed stream, or an edited
`restore_to`, makes nothing at the provider.

Modules touched: the kernel (a new port, `OutboundDestinations`, and a
destination argument on `OutboundRequests::stored_send`), `connectors`
(implements the port, binds outbound changes and restore places, refuses
mismatches) and `conversations` (binds the destination into both send
MACs). One migration. No new endpoint, event, scope, job, dependency or
secret.

## What Changes

- **Where a send goes is bound.** The author's request (`drafts.request_mac`)
  and the stored send (`drafts.send_mac`) gain the destination (connection,
  stream, mailbox) as the connector resolves it, under new labels
  (`campbooks/conversations.send-request/v2`, `…stored-send/v2`).
  `send_draft` signs it when the member asks; `release_send` resolves it
  again and releases only when it is the same, then signs the stored send
  for it; `queue_send` and every `send_message` attempt pass the destination
  they would send through to the outbound-requests port, which says the
  send does not stand when the MAC names another.
- **Where a change is made is bound.** The outbound-change MAC
  (`connectors.outbound_changes.mac`) gains the stream's connection and
  mailbox, under `campbooks/connectors.outbound-change/v2`, signed by
  `request_changes` for the destination it resolved (one stream only for the
  inbox) and verified by each run for the connection and mailbox it loaded.
- **Where a trashed message goes back is bound.** When a run records Zoho's
  `restore_to` for a trash change it made, it signs the change again with
  the place included (only while the change still verifies as recorded);
  an untrash or not-spam whose place does not verify is refused.
- **Refusals use the existing paths.** A send that does not stand fails
  `unavailable` without a provider call; a change that does not verify ends
  `skipped` (`unverified`) and its message is re-read; a scheduled draft
  whose request does not verify is left as it is. Each logs a WARN with ids
  only. Nothing retries a refused row.
- **What was in flight at the deploy.** The migration records each messages
  stream's inbox, connection and mailbox
  (`connectors.destinations_at_deploy`) and each outbound change's
  `restore_to` (`connectors.restores_at_deploy`), in tables the runtime role
  may read and never write. A `…/v1` MAC still verifies, but only while the
  destination resolved now is the one recorded there (and, for a restore
  place, while it is the one recorded there).

## Out of scope (follow-ups listed in design.md)

- A stream repointed *before* the member asks: the binding records what the
  rows say at that moment. Binding the stream rows themselves at grant time
  closes it.
- Change requests are bound to their destination when the connector records
  them, seconds after the member's request, not in the member's request.
- Zoho's folder ids (Inbox, Trash, Spam and the listed folders) come from the
  stream's cursor, which SQL can edit.
