# Design

## 1. Inventory: what acts at the provider because of an event

Every consumer and job of the connectors module, and whether it sends,
moves, labels, deletes or archives mail at the provider:

| Handler | Trigger | Action at the provider | Now |
|---|---|---|---|
| `connectors.queue_send` | `conversations.send_requested` (event) | none itself; records the send and enqueues `send_message` | asks `stored_send` first; records the port's document; nothing when the send does not stand or names another inbox |
| `connectors.send_message` | job enqueued by `queue_send` (and its `give_up` check) | sends the message (Gmail `messages.send`, Zoho send or reply), or looks an unclear attempt up | asks `stored_send` again before any provider call, in a short transaction of its own; sends the port's document, which must equal the recorded one; settles the send `unavailable` (or `unknown_outcome` if unclear) when it does not stand |
| `connectors.request_changes` | `conversations.changes_requested` (event) | none itself; records outbound changes and runs the stream, after asking Gmail for a history stamp (a read) | asks `changes_standing` first; records only the items that stand; asks for no stamp when none does |
| `connectors.sync_stream` | job (schedule, `run_now`) | makes the recorded outbound changes (labels, flags, inbox) before it reads | unchanged: it makes only rows `request_changes` recorded (see §4) |
| `connectors.revoke_departed_grants` | `workspaces.member_left` | revokes the grant (no mail changes) | unchanged (re-checked since #211) |
| `connectors.revoke_workspace_grants` | `workspaces.workspace_deleting` | revokes every grant (no mail changes) | unchanged (re-checked since #211) |
| `connectors.dispatch_due` | schedule (instance-only) | none; enqueues stream runs | unchanged |

No other module talks to a provider. `release_send` and `check_send` are
conversations jobs; they publish and settle, they call nobody.

## 2. The port

`kernel::outbound` (the kernel knows no mail; the port speaks of a
"container" inbox, a document blob and opaque change items):

- `stored_send(state, conn, workspace, send) -> Option<StoredSend { inbox, message }>`
- `changes_standing(conn, workspace, inbox, conversation, items) -> Vec<bool>`

`NoOutboundRequests` (the default) answers `None` and all `false`, so a
server that forgot to register the port sends and changes nothing. The
server registers `campbooks_conversations::outbound_requests()` in
`with_ports`, which the API and the worker both use.

Considered: a synchronous call from connectors into conversations' tables.
Refused by `cargo xtask arch` (modules don't import modules, no cross-schema
reads) and by ADR 0012. An event round trip (connectors asks, conversations
answers with an event) would leave the same question for the answering
event, and add latency to every send.

### What the conversations module answers

**A send stands** when, read in the asking transaction:

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
document to equal the one it recorded.

**A change stands** when the conversation is in the event's inbox, the
item parses as a requested change, its message belongs to that
conversation, and the stored state shows it:

| Change | Stands when |
|---|---|
| `mark_read` / `mark_unread` | the message is read / unread |
| `star` / `unstar` | the message is starred / not starred |
| `archive` / `unarchive` | the message is out of / in the inbox |
| `add_label` | the label is on the conversation, and the label of that inbox has exactly the provider key named (none when none) |
| `remove_label` | the label is not on the conversation, and the label of that inbox has exactly the provider key named |
| `move_to_stream` | the conversation is in that stream |

A member's change is applied in the transaction that publishes the request,
so it always stands when the connector asks, unless a later change undid it.
Then the later change stands in its place and the provider ends as the
member left it: read then unread before the connector runs records only
the unread (the provider had it unread already). An unknown kind (a later
contract version) does not parse here and does not stand; the connector
skipped those anyway (`unsupported`). The match over the kinds has no
wildcard, so a kind added to the contract fails to compile until it says
when it stands.

## 3. Content comes from the stored message

`send_requested.message` names a blob, and any workspace transaction can
store a blob of its own. The connector no longer sends it: `queue_send`
records the port's document, and `send_message` sends the port's document
after checking it equals the recorded one. The event keeps the field
(contract unchanged); a forged one changes nothing. For changes, the label
key is checked against the stored label, so a forged key is not written.

## 4. Isolation, per table

| Table | Read or written by | Scope |
|---|---|---|
| `conversations.drafts` (new column `send_document`) | the port (`FOR SHARE`), `release_send` (written with `send_id`) | the asking transaction: `queue_send`'s delivery transaction, or a short `begin_for(workspace)` in `send_message` and `request_changes`; row-level security as before |
| `conversations.conversations`, `messages`, `conversation_labels`, `labels` | the port (plain reads) | the same transactions |
| `connectors.sends` | `queue_send`, `send_message` | unchanged |
| `connectors.outbound_changes` | `request_changes`, `sync_stream` | unchanged |

Rows written straight into `connectors.sends` (with a forged
`connectors.send_message` job) are covered: `send_message` asks the port
before every provider call. Rows written straight into
`connectors.outbound_changes` are not re-checked at push time: a
transaction that can write them can as well write the conversations tables
the port reads (the mail state, labels), and the push has no conversation
id to ask about. Left as is, see "Left open".

## 5. Idempotency

Unchanged guards, now tested against replays:

- `connectors.sends` is keyed by send id (`ON CONFLICT DO NOTHING`), so a
  replay while the draft is still `sending` records and enqueues nothing;
- `send_message` acts only on a `pending` send, and `finish_sent` marks it
  sent once (`state <> 'sent'`);
- once the send is over, its draft is deleted (sent) or `failed`, so the port
  no longer confirms it and a replay is refused before anything is recorded;
- `outbound_changes` is keyed by (request id, message): a replayed change
  request records nothing twice.

## 6. On a mismatch

`queue_send` and `request_changes` complete the delivery with a WARN
(`send_requested names no send that stands; nothing is sent`,
`changes_requested names changes its conversation does not show; those are
not made`) carrying the workspace and send ids, or counts, never content or
addresses. They don't retry. `send_message` logs `a recorded send no longer
stands; not sending` and settles the send as failed, so it is not left
`pending` for the give-up check and the draft, if there is one, becomes
editable again. A port error (the database) is retried like any other.

## 7. Migration

`20261021190000_conversations__draft_send_document.sql`: the first free slot
after `next`'s newest (`20261021170000`) and the open PR #540's
(`20261021180000`–`20261021180200`). `ALTER TABLE conversations.drafts ADD
COLUMN send_document jsonb` (nullable, no default: a catalog-only change),
with a 10 s lock timeout. No backfill: a draft `sending` when the
migration runs was released by the older image, so it has no document (see
§8).

### Rollback SQL

sqlx refuses to start with an applied migration it does not know. Before
redeploying an image older than this change, run once as the migration
login:

```sql
-- Roll back campbooks-cloud#226's column BEFORE redeploying an older image.
BEGIN;
SET LOCAL lock_timeout = '10s';
ALTER TABLE conversations.drafts DROP COLUMN send_document;
DELETE FROM _sqlx_migrations WHERE version = 20261021190000;
COMMIT;
```

The older image never reads the column, so keeping it is harmless if the
migration row is the only problem; dropping it keeps the schema equal to
`next`'s.

## 8. Rolling deploys

- An older worker still running `release_send` after the migration marks a
  draft `sending` without a document. The new `queue_send` does not record
  that send (the port says it does not stand), so it is not sent; the
  draft fails `unknown_outcome` after the hour `check_send` waits and can be
  sent again. The window is the overlap of the two workers (seconds). Run
  the API and the worker of the same version.
- An older worker running `queue_send` records the event's document
  unchecked, as today, until it is replaced; the new `send_message` then
  asks the port, which refuses a send released by an older image (no
  document), so a send recorded across the overlap may fail `unavailable`.

## Prod risks

- **Background work that could stop.** Every send and every member's
  change now needs one more short read in the conversations tables (one row
  by unique `send_id`; for changes, four small reads by conversation and
  primary keys). A port error retries the delivery or the job, never acts.
  A mismatch is final for that delivery: a legitimate send refused by a bug
  in the port would fail `unavailable` (the member sees the draft failed and
  can send again), not vanish silently. A legitimate change refused that
  way would not reach the provider, and the next sync would bring the
  provider's state back.
- **Membership at send time.** `send_message` now checks the author is
  still a member who may send. An author removed between the release and
  the call (seconds) has their send failed instead of sent, which is what
  `release_send` already does one step earlier.
- **Stale requests.** A change request whose change was undone before the
  connector read it records nothing; the undo's own request carries the
  final state. Before, both were recorded and the later one superseded the
  earlier at the run.

## Left open

- Re-checking `connectors.outbound_changes` at push time (§4).
