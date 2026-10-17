# Design

## 1. Inventory: what acts at the provider because of an event

Every consumer and job of the connectors module, and whether it sends,
moves, labels, deletes or archives mail at the provider:

| Handler | Trigger | Action at the provider | Now |
|---|---|---|---|
| `connectors.queue_send` | `conversations.send_requested` (event) | none itself; records the send and enqueues `send_message` | asks `stored_send` first; records the port's document; nothing when the send does not stand or names another inbox |
| `connectors.send_message` | job enqueued by `queue_send` (and its `give_up` check) | sends the message (Gmail `messages.send`, Zoho send or reply), or looks an unclear attempt up | asks `stored_send` again, in a short transaction of its own, before every provider call; never sends a send that does not stand (it may still look an unclear attempt up, a read); claims each attempt atomically; uses a stream only of the send's own inbox |
| `connectors.request_changes` | `conversations.changes_requested` (event) | none itself; records outbound changes and runs the stream, after asking Gmail for a history stamp (a read) | asks `changes_standing` first; records only the items the conversations module recorded under that event's id; asks for no stamp when none does |
| `connectors.sync_stream` | job (schedule, `run_now`) | makes the recorded outbound changes (labels, flags, inbox) before it reads | unchanged: it makes only rows `request_changes` recorded (see "Left open") |
| `connectors.revoke_departed_grants` | `workspaces.member_left` | revokes the grant (no mail changes) | unchanged (re-checked since #211) |
| `connectors.revoke_workspace_grants` | `workspaces.workspace_deleting` | revokes every grant (no mail changes) | unchanged (re-checked since #211) |
| `connectors.dispatch_due` | schedule (instance-only) | none; enqueues stream runs | unchanged |

No other module talks to a provider. `release_send` and `check_send` are
conversations jobs; they publish and settle, they call nobody.

## 2. The port

`kernel::outbound` (the kernel knows no mail; the port speaks of an inbox,
a document blob and opaque change items):

- `stored_send(state, conn, workspace, send) -> Option<StoredSend { inbox, message }>`
- `changes_standing(conn, workspace, request, inbox, conversation, items) -> Vec<bool>`

`NoOutboundRequests` (the default) answers `None` and all `false`, so a
server that forgot to register the port sends and changes nothing. The
server registers `campbooks_conversations::outbound_requests()` in
`with_ports`, which the API and the worker both use.

Considered: a synchronous call from connectors into conversations' tables.
Refused by `cargo xtask arch` (modules don't import modules, no cross-schema
reads) and by ADR 0012. An event round trip (connectors asks, conversations
answers with an event) would leave the same question for the answering
event, and add latency to every send.

### A send stands

Read in the asking transaction:

1. a draft has `send_id = send` (row-level security limits it to the
   workspace), locked `FOR SHARE` until that transaction ends, so the
   outcome (`record_send`, `FOR UPDATE`) waits for it;
2. its state is `sending` (not `draft`, `scheduled`, `failed`, nor gone:
   sent drafts are deleted, cancelled ones are back to `draft`);
3. `send_document` holds the blob reference `release_send` recorded;
4. its author is a member (`Membership::role_held`, held until the
   transaction ends) allowed to `send` the draft (`authz::authorize`, the
   draft's facts: their own draft, in an inbox they may still send from).
   The same function `release_send` uses (`author_may_send`). A draft with
   no author (a client token's) never stands, as `release_send` never
   releases one.

The connector then also requires the port's inbox to equal the event's
inbox (and, in `send_message`, the recorded send's inbox), and the port's
document to equal the one it recorded. `send_message` reads the recorded
send's stream only when that stream feeds the send's own inbox
(`st.inbox_id = s.inbox_id`); a row pointed at another mailbox's stream has
none and fails `not_connected`.

### A change stands

`conversations::domain::conversation::publish_request` is the only place
`changes_requested` is published: `request` (one change for many messages:
labels, a move, rules) and `request_each` (each message with its own change:
the flags, read, star, archive, trash and spam) both go through it. It
records each request in `conversations.change_requests` under the event's
id (`EventBus::publish` returns it), in the same transaction: the inbox,
the conversation and the items. The port answers, per item, whether the
row of that id, conversation and inbox holds an equal item (the item is
parsed on both sides, so key order or a missing optional field doesn't
matter; `untrash` and `unspam` carry `inbound`, which the connector always
writes back and which must match). A forged
event gets a fresh id with no row: none of its items stands, even one that
matches what the conversation already shows. An unknown kind (a later
contract version) does not parse and does not stand; the connector skipped
those anyway (`unsupported`).

Considered first: answering from the conversation's current state (the
message is read for `mark_read`, and so on). Dropped: a sync batch fetched
before a member's change and applied after it (`apply_changes` writes the
provider's flags) makes the change no longer "stand", and it would be lost
for good, where the outbound row used to carry it to the provider. Checking
the recorded request keeps today's behaviour (a change and its undo are
both recorded, the run merges them, the later one wins) and refuses only
requests this module never made.

Rows are deleted with their conversation (`ON DELETE CASCADE`, so with
their inbox, a disconnect and a workspace purge), and rows older than seven
days are deleted when the next request is made on the same conversation.
There is no other sweep: a conversation that sees no further request keeps
its last rows (ids and provider message ids only) until it goes. The
connector asks within seconds; a delivery still retrying after seven days
may find its row gone and record nothing (its outbound change would have
expired at the run anyway).

## 3. Content comes from the stored message

`send_requested.message` names a blob, and any workspace transaction can
store a blob of its own. The connector no longer sends it: `queue_send`
records the port's document, and `send_message` sends the recorded document
only while the port confirms the send with that same document. The event
keeps the field (contract unchanged); a forged one changes nothing. For
changes, only items equal to the recorded ones are made, so a forged label
key or message id is not written.

## 4. Isolation, per table

| Table | Read or written by | Scope |
|---|---|---|
| `conversations.drafts` (new column `send_document`) | the port (`FOR SHARE`), `release_send` (written with `send_id`) | the asking transaction: `queue_send`'s delivery transaction, or a short `begin_for(workspace)` in `send_message`; row-level security as before |
| `conversations.change_requests` (new) | `conversation::request` (insert, prune), the port (read) | the publishing transaction; `request_changes`' first `begin_for(workspace)`; forced row-level security, `workspace_isolation`; the runtime role has the usual four privileges (it only selects, inserts and deletes) |
| `connectors.sends` | `queue_send`, `send_message` (attempts now claimed with a guarded `UPDATE … RETURNING`) | unchanged |
| `connectors.outbound_changes` | `request_changes`, `sync_stream` | unchanged |

## 5. Idempotency

- `connectors.sends` is keyed by send id (`ON CONFLICT DO NOTHING`), so a
  replay while the draft is still `sending` records and enqueues nothing;
- `send_message` acts only on a `pending` send, and `finish_sent` marks it
  sent once (`state <> 'sent'`);
- every call to the provider is now preceded by an atomic claim: the
  attempt is recorded only while the send is `pending` and its
  `attempted_at` is still what this run read. Of two jobs of one send (a
  duplicate, a planted row) one calls the provider; the other waits and
  then finds the outcome, or an unclear attempt to look up. Before, both
  recorded an attempt and both called;
- clearing the unclear mark (a lookup proved the attempt unsent, or the
  provider refused it) is guarded the same way: only while the last attempt
  is still the one this run read or made, so a slow runner never clears a
  newer claim and opens the way to a second send;
- once the send is over, its draft is deleted (sent) or `failed`, so the port
  no longer confirms it and a replay is refused before anything is recorded;
- `outbound_changes` is keyed by (request id, message): a replayed change
  request records nothing twice.

## 6. On a mismatch

`queue_send` and `request_changes` complete the delivery with a WARN
(`send_requested names no send that stands; nothing is sent`,
`changes_requested names changes the conversations module did not request;
those are not made`) carrying the workspace and send ids, or counts, never
content or addresses. They don't retry. `send_message` logs `a recorded
send no longer stands; not sending` and settles the send as failed, so it
is not left `pending` for the give-up check and the draft, if there is one,
becomes editable again. An unclear send that no longer stands is still
looked up first: one that went out is recorded `sent` (so a draft that
`check_send` already failed, or whose author left, still learns it went),
and only one proven unsent is failed `unavailable`. The lookup uses the
port's document (its `Message-ID`) whenever the port returns the send, and
the row's own only when the port returns nothing; a message found while the
send does not stand is recorded `sent` without indexing or announcing the
bytes this module built, so a tampered row's content never reaches a
conversation. A port error (the
database) is retried like any other.

## 7. Migrations

The first free slots after `next`'s newest (`20261021180500`, from #540) and
every open PR's (#542: `20261021190000`):

- `20261021200000_conversations__draft_send_document.sql`: `ALTER TABLE
  conversations.drafts ADD COLUMN send_document jsonb` (nullable, no default:
  a catalog-only change). No backfill: a draft `sending` when the migration
  runs was released by the older image, so it has no document (see §8).
- `20261021200100_conversations__change_requests.sql`: the new table, its
  index, forced row-level security and grants.

Both with a 10 s lock timeout.

### Rollback SQL

sqlx refuses to start with an applied migration it does not know. Before
redeploying an image older than this change, run once as the migration
login:

```sql
-- Roll back campbooks-cloud#226 BEFORE redeploying an older image.
BEGIN;
SET LOCAL lock_timeout = '10s';
DROP TABLE conversations.change_requests;
ALTER TABLE conversations.drafts DROP COLUMN send_document;
DELETE FROM _sqlx_migrations WHERE version IN (20261021200000, 20261021200100);
COMMIT;
```

The older image never reads either, so keeping them would be harmless if
the migration rows were the only problem; dropping them keeps the schema
equal to `next`'s.

## 8. Rolling deploys

- An older worker still running `release_send` after the migration marks a
  draft `sending` without a document, and that send is never made:
  - when the new `queue_send` gets it, it records nothing (the port says it
    does not stand), and the draft fails `unknown_outcome` after the hour
    `check_send` waits;
  - when an older `queue_send` had already recorded it, the new
    `send_message` settles it `unavailable` at once (nothing went out) and
    the draft is editable again.

  Either way it can be sent again. The window is the overlap of the two
  workers (seconds). Run the API and the worker of the same version.
- An older API or worker publishing a change request records no row, so the
  new `request_changes` does not make it at the provider; the next sync
  then brings the provider's state back. Same window.
- An older `queue_send` records the event's document unchecked until it is
  replaced; the new `send_message` still asks the port before calling (see
  the first point for a send the older image released).

## Prod risks

- **Background work that could stop.** Every send and every member's
  change now needs one more short read in the conversations tables (a row
  by unique `send_id`, or by the request's primary key), and every change
  request writes one small row (ids and provider message ids). A port
  error retries the delivery or the job, never acts. A mismatch is final
  for that delivery: a legitimate send refused by a bug in the port would
  fail `unavailable` (the member sees the draft failed and can send again),
  not vanish silently. A legitimate change refused that way would not reach
  the provider, and the next sync would bring the provider's state back.
- **Membership at send time.** `send_message` now checks the author is
  still a member who may send. An author removed between the release and
  the call (seconds) has their send failed instead of sent, which is what
  `release_send` already does one step earlier. An unclear attempt is still
  looked up, so one that went out is recorded as sent.
- **A second job of one send** now waits and retries (30 s) instead of
  calling the provider again.

## Left open

- **SQL in the workspace can still make it send.** This change makes sure
  the event payload is never what gets sent or changed, and that an event
  alone is not enough. It does not stop an attacker who can run SQL as the
  runtime role in workspace W: they can skip the event, insert a
  `scheduled` draft with content of their own and a member allowed to send
  as its author, and plant a `conversations.release_send` job; the release
  then publishes, the port confirms it, and the mail goes out. The same
  holds for change requests (insert a `change_requests` row with the
  event). So "a forged request never sends mail no member asked for" does
  not hold against SQL in W; only "the event payload is never what gets
  sent" does. The fix is a binding the database cannot forge (a MAC over
  the release, keyed outside the database): campbooks-cloud#227.
- Rows written straight into `connectors.outbound_changes` are not checked
  again at push time, for the same reason.
