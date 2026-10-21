# Design

## Threat model

SQL as the runtime role in workspace W, without the instance secret. It can
read and write every row of W, append W's events and plant W's jobs. Since
campbooks-cloud#227 and #242 items 1–3, 8–19 it cannot make a send or a
change verify that no member asked for. This change takes away where a real
request goes:

- **(4)** repointing a stream after the member asked, so a signed send or
  change goes out through another connection, or another mailbox of the same
  connection (Zoho has one stream per mail account of a grant);
- **(5)** editing Zoho's `restore_to`, so an untrash moves the message where
  SQL chose.

## Inventory

Line numbers are on this branch.

### Where a send or an outbound change is signed

| What | Where | MAC |
| --- | --- | --- |
| The author's request to send | `conversations/src/features/send_draft/mod.rs:212` `bind_request` | `drafts.request_mac` (`SendRequest`) |
| The stored send | `conversations/src/features/release_send/mod.rs:89-91` | `drafts.send_mac` (`StoredSend`) |
| A change request | `conversations/src/domain/conversation.rs:416` `publish_request` (`:463`) | `change_requests.mac` (`ChangeRequest`) |
| An outbound change | `connectors/src/features/request_changes/mod.rs:182` → `connectors/src/domain/outbound.rs:533` `bind` | `outbound_changes.mac` |
| A restore place (new) | `connectors/src/domain/outbound.rs:939` `bind_restores` (from `record_outcome`, `:875`) | `outbound_changes.mac` again |
| Re-tags at a key rotation | `conversations/src/features/retag_bindings/mod.rs:296`, `:356`; `connectors/src/domain/outbound.rs:468` `retag` | the same columns |

### Where inbox → stream → connection is resolved before a provider call

| Where | What it resolves | Now |
| --- | --- | --- |
| `conversations/.../send_draft/mod.rs:212` | the inbox's destination, through the new port | signed into the request |
| `conversations/.../release_send/mod.rs:154` `releasable` | the same, again | must equal the request's; signed into the stored send |
| `connectors/src/features/queue_send/mod.rs:55` | `destinations::of_inbox` (was `streams WHERE inbox_id`) | passed to `stored_send`; the send row records that stream |
| `connectors/src/features/send_message/mod.rs:579` `load` → `:634` `standing_document` | `sends.stream_id` → `streams` (inbox matched) → `connections`, and the stream's `mailbox_key` (Zoho's account, `via_zoho.rs:74`) | passed to `stored_send` before every attempt |
| `connectors/src/features/request_changes/mod.rs:82` (the target) and `:96` | `streams WHERE inbox_id` → `connections`, and `of_inbox` (exactly one stream) | must agree; signed into each outbound change |
| `connectors/src/features/sync_stream/mod.rs:1161` `load` → `:140` `backlog`, `:221` `record_outcome` | the stream's connection (whose token the run uses) and mailbox | each change must verify for them |
| `connectors/src/domain/outbound.rs:783` `restore_of` | Zoho's `restore_to` of the latest trash change | must verify; else the untrash is refused |

## Decisions

### The destination is (connection, stream, mailbox)

The connection decides whose grant (token) acts; the stream decides which
cursor, labels and index; the mailbox key decides which Zoho account a call
names (one connection can hold several). Binding only the connection would
leave a swap between two mail accounts of one Zoho grant open. Credentials
are sealed with associated data naming their connection, so a connection id
also pins the token.

### A kernel port tells conversations where an inbox's mail goes

Conversations signs sends but owns no stream. Modules do not import modules,
so the connectors module answers through a new kernel port,
`OutboundDestinations::destination(conn, inbox)`, registered by the server
(`with_ports`). Without it (a module's own tests) no inbox goes anywhere: the
send is bound to "no destination", and the connector, which always resolves
one, refuses it. Fail closed.

The connector does not let conversations re-resolve the destination when it
asks whether a send stands: it passes the destination it is about to use
(`stored_send(…, at)`), so the check is against exactly the stream and
connection of the call, including `sends.stream_id`, never a second lookup
that SQL could make disagree with the first.

### More than one stream claiming an inbox resolves to nothing

`streams.inbox_id` has no unique constraint (only SQL makes two). Whichever
row a query happened to return would be SQL's choice, so `of_inbox` answers
`None` when two claim the inbox (a WARN), and `request_changes` records
nothing then.

### Change requests are bound where the connector records them

Binding the destination into `change_requests.mac` would mean resolving it in
`conversation::publish_request`, reached from the rule engine, the decide
pipeline, stream mirroring and every member operation, all of which carry a
`Binding` and an `EventBus` but no state. The connector records a change
seconds after the request (the event's consumer); from then on the outbound
change's MAC names the destination. The window before that is the same class
as a stream repointed before the member asked (below).

### What was in flight at the deploy: v1 verified only for the recorded destination

Two options were weighed (the brief's):

1. **Re-sign at the deploy** (as `rule_resigns`): a job signs each v1 row as
   v2 with the destination resolved then. That needs a definer function to
   tie the job to rows the migration saw, a dispatcher and a job, and the
   destination it would sign is still read from rows SQL controls at that
   moment.
2. **Keep v1 verifying, re-check the destination against the deploy's
   record.** Chosen. The migration copies each messages stream's
   (stream, inbox, connection, mailbox) into
   `connectors.destinations_at_deploy`, and each `restore_to` into
   `connectors.restores_at_deploy`, read-only for the runtime role (no
   `INSERT`/`UPDATE`/`DELETE` grant, forced row-level security). A v1 MAC
   verifies only when the destination resolved now equals the recorded one
   (`Resolved::as_at_deploy`), and a v1 restore place only when it also
   equals the recorded place.

SQL never chooses which version is verified: a v2 tag never verifies as v1
(different label), SQL cannot mint tags, and it cannot add or change a row
of the records. The worst it can do is delete a stream or a change, which
removes its record with it (cascade) and makes the v1 row refuse. Rows
written by the old version during a rolling deploy, after the migration, are
v1 for destinations that are recorded, so they still work while nothing is
repointed. No row needs a member's action and no background job is added.

The records can be dropped by a later migration once nothing v1 is left
(sends go out within minutes, scheduled ones within their schedule, pending
changes expire with `batch_retention_days`).

### The restore place is signed into the trash change after it is made

`restore_to` is learned from Zoho when the trash is made, after the change
was signed. `record_outcome` re-signs the trash change with the place
(`OUTBOUND_CHANGE_V2` with the place present), only while the change still
verifies as recorded under its row (a compare-and-set on the old MAC), so an
edit between the run's check and its record is not laundered into a valid
MAC. `restore_of` verifies the place before the untrash; an untrash or
not-spam (out of the trash or spam and into neither) of a message whose
place does not verify is refused like any unverified change. Re-tagging at
a key rotation now also covers applied changes holding a place.

### Refusals

| What | Where | Result |
| --- | --- | --- |
| Scheduled draft, destination changed | `release_send` `releasable` | not released, draft left scheduled, WARN, job ends (no retry) |
| Stored send, destination changed, before the connector recorded it | `queue_send` | nothing recorded, WARN, consumer ends |
| Recorded send, destination changed before an attempt | `send_message` `standing_document` → `not_standing` | `failed` (`unavailable`), no provider call; an unclear earlier attempt is still only looked up |
| Outbound change, stream's connection or mailbox changed | `outbound::verified` | `skipped` (`unverified`), message re-read, WARN |
| Untrash with a place that does not verify | `outbound::backlog` | `skipped` (`unverified`), message re-read, WARN |
| Inbox claimed by two streams | `request_changes`, `queue_send` | nothing recorded, WARN |

None of these leaves a job retrying: they end the job successfully (as the
existing MAC mismatches do), and a skipped change is never pending again.

## Other providers' targets

- **Gmail:** label ids are in the signed `add_labels`/`remove_labels`; a move
  to a stream is mapped to labels when recorded; `messages.untrash` restores
  Gmail's own labels (no place stored). The send's thread key is in the
  outgoing document, which the stored send binds. Nothing left unsigned at
  the point of the call.
- **Zoho, not fixed here (follow-up):** the Inbox, Trash and Spam folder ids
  and the list of folders a remembered place must be in come from the
  stream's cursor (`streams.cursor`), which SQL can edit between runs. Its
  impact is a move into another folder of the same account.
- **Zoho `provider_server`:** only a configured data centre is used
  (`Zoho::centre`), so SQL cannot point calls at another host.
- **Before binding (follow-up):** which labels are stream marks, the label
  ids known (`gmail_labels`, `zoho_labels`) and the stream an inbox resolves
  to are read from rows when the request is made. Binding the stream rows
  themselves when a grant creates them (a MAC over stream, connection,
  inbox, mailbox) would close the "repointed before the member asked" case
  for every path at once.

## Migration

`20261023100000_connectors__destinations_bound.sql` (slot 100000–100200).
Two new tables, created and filled in the migration's transaction (row-level
security lifted for the snapshot statements only, as in `rule_resigns`),
`SELECT` granted to `campbooks_app`. No column of an existing table changes
and no MAC is rewritten, so there is no data to convert back.

Rollback:

```sql
DROP TABLE IF EXISTS connectors.restores_at_deploy;
DROP TABLE IF EXISTS connectors.destinations_at_deploy;
DELETE FROM _sqlx_migrations WHERE version = 20261023100000;
```

Rolling the code back after v2 rows were written: the old version does not
verify v2 MACs, so sends released and changes recorded by the new version
would be refused (failed `unavailable`, skipped `unverified`) and members
would send or change them again. Nothing is sent or changed wrongly.
