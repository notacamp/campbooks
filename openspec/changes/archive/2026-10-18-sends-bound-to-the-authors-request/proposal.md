# Proposal

## Why

The review of public #543 (campbooks-cloud#226) left one gap, tracked as
campbooks-cloud#227. #543 made the connector re-check the stored draft or
change request before it acts, so an event alone no longer sends or changes
mail. But an attacker with SQL as the runtime role inside workspace W (not
the server's secret-key file) can skip the event: insert a `scheduled`
draft with their own recipients and body, name as author a member who may
send from the inbox, give it a matching `send_generation`, and plant a
`conversations.release_send` job. The release then writes the document,
marks the draft `sending` and publishes the request; every check passes and
the mail goes out from W's mailbox. The same holds for a planted
`conversations.change_requests` row (and its event), and for a row written
straight into `connectors.outbound_changes`.

After this change, SQL alone cannot make any mailbox send, or make a change
at the provider.

Modules touched: the kernel (a binding key and its canonical encoding),
`conversations` (signs at the author's request, verifies at release and in
the outbound-requests port), `connectors` (signs the outbound changes it
records, verifies before a run makes them), the server (derives the key at
startup). Two migrations. No new endpoint, event, scope, dependency or
secret.

## What Changes

- **The binding key** (`kernel::binding::Binding`): HMAC-SHA256 under a key
  derived (HKDF-SHA256, label `campbooks/send-binding/v1`) from the existing
  instance secret (`CAMPBOOKS_SECRET_KEY[_FILE]`). Never stored, logged or
  sent. Tags are a version byte and the HMAC, compared in constant time;
  inputs are canonical (`kernel::binding::Fields`: a versioned context
  label, then each field length-prefixed). Without an instance secret
  nothing is signed and nothing verifies: sends are refused (500 at
  `send_draft`) and nothing goes out, with a WARN at startup.
- **The author's request is signed.** `conversations.send_draft` stores
  `drafts.request_mac` over the workspace, draft, send generation, author,
  inbox and a hash of everything that goes out (sender, To/Cc/Bcc, subject,
  body, quoted history, reply headers, thread, what it answers, each
  attachment with its blob hash, size and type).
- **The release verifies it before it writes anything.**
  `conversations.release_send` refuses a draft whose binding does not verify:
  no document, no state change, a WARN with ids only, no retry. It then
  stores `drafts.send_mac` over the workspace, send id, inbox, author and the
  outgoing document's blob, and clears the spent `request_mac`. Cancelling a
  send clears it too.
- **The port verifies the stored send.** The outbound-requests port answers
  that a send stands only when its `send_mac` verifies; the connector already
  refuses a send that does not stand.
- **Change requests are signed where they are recorded**
  (`change_requests.mac`, over the workspace, request, inbox, conversation
  and items) and the port verifies that before any item stands.
- **Outbound changes are signed by the connector** when it records them,
  after the port confirmed the request (`outbound_changes.mac`, over the
  workspace, row, stream, request, message, kind and labels), and a stream's
  run verifies each before it makes it: one that does not verify ends
  `skipped` (`unverified`) and its message is re-read from the provider.
- **At deploy**, drafts still `scheduled` (no binding) go back to drafts in
  the migration, so their authors send them again; a NULL binding never
  verifies anywhere.
- The kernel port's `changes_standing` takes the app state (for the key).

## Non-goals

- **Replaying an authentic request.** SQL can restore a row exactly as a
  member's request left it (a send the author cancelled, while the binding
  was readable). The binding guarantees that what goes out is something the
  author asked to send, from that inbox, to those recipients; it cannot tell
  a restored row from the original. The spent binding is cleared at release
  and on cancel, which shortens the window to the undo window. See
  `design.md`.
- **Rules and automation.** A change a taught rule makes is signed as made
  by the server; SQL that plants a rule is out of scope here.
- **Key rotation.** Designed in `design.md`, not built.

## Impact

- SQL in a workspace can no longer make its mailbox send, or change mail at
  the provider, by planting or editing rows.
- A draft scheduled just before the deploy goes back to a draft (sent again
  by its author); one `sending` at the deploy fails as an unconfirmed send;
  a change pending at the deploy is not made (the next sync shows the
  provider's state). The window is the deploy itself (seconds).
