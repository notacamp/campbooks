# Design

## 1. Threat model

An attacker has SQL as the runtime role inside workspace W (row-level
security holds them to W), but not the server's secret-key file. Since
#543 (campbooks-cloud#226) an event alone sends or changes nothing: the
connector asks the conversations module, through the outbound-requests
port, whether its own records say so. But those records are rows W's
runtime role can write. Before this change the attacker could:

1. insert a `scheduled` draft with their own recipients and body, a member
   who may send from the inbox as its author and a matching
   `send_generation`, and plant a `conversations.release_send` job: the
   release wrote the document, marked the draft `sending`, published, and
   the port confirmed it;
2. edit a member's scheduled draft (recipients, body, attachments) inside
   its undo window;
3. plant a `sending` draft with a `send_id`, any document blob W holds, and
   the matching `send_requested`;
4. plant a `conversations.change_requests` row with the matching
   `changes_requested`, or edit a recorded one's items;
5. insert or edit a pending row of `connectors.outbound_changes`, which the
   stream's next run made at the provider.

After this change none of these makes a mailbox send or change anything,
because each record carries a MAC under a key the database never holds.

## 2. The key

`kernel::binding::Binding` holds an HMAC-SHA256 key derived from the
instance secret (`kernel::crypto::SecretKey`, from
`CAMPBOOKS_SECRET_KEY_FILE` or `CAMPBOOKS_SECRET_KEY`, the secret the
`Signer`, the BFF and the connectors' credential cipher already use) with
HKDF-SHA256, an empty salt and the label `campbooks/send-binding/v1` as
`info` (the same derivation as `crypto::derive_secret`, domain-separated by
its own label). No new secret. The key is never written to the database,
a log, an error or an event; `Binding`'s `Debug` prints `Binding(<key>)`.

The server derives it at startup (`main.rs`, `binding()`) and installs it
with `AppState::with_binding`; the API and the worker of one install agree
because they read the same secret. **Fail closed:** without an instance
secret, `Binding::disabled()` signs nothing and verifies nothing. The server
logs a WARN at startup (`CAMPBOOKS_SECRET_KEY is not set: sends and changes
at the mail provider are refused …`), `send_draft` answers 500 (a WARN with
ids), and a release, a stored send, a change request and an outbound change
never verify. In practice an instance with a connected mailbox always has
the secret: connectors refuse to start with a provider configured and no
secret (they need it to seal credentials).

Considered: a random per-process key (the `Signer`'s fallback). Rejected:
the API signs and the worker verifies, and a restart between the two would
fail every send in flight; the brief also asks for an explicit fail-closed.
Considered: keys in the database (pgcrypto). Rejected: that is the
attacker's reach. Considered: asymmetric signatures (the API signs, the
worker only verifies). No gain here: both processes hold the instance
secret anyway (the worker unseals the mailbox credentials with it).

## 3. Tags and encoding

A tag is `0x01 || HMAC-SHA256(key, input)` (33 bytes, `TAG_VERSION` first
so the format can change), compared with `hmac::verify` (constant time).
A tag of another version, a truncated tag or none (NULL) never verifies.

The input is a `kernel::binding::Fields`: a context label naming the use and
its version (`campbooks/conversations.send-request/v1`, …), then each field
as an 8-byte big-endian length and its bytes. Options are a presence byte
then the value; lists are a count then the items. So no two requests share
an input (`["ab","c"]` vs `["a","bc"]`, `None` vs `Some("")`), and two uses
never share one (the label). The derivation and the encoding are pinned by
a known-answer test computed with an independent implementation (Python's
`hmac`/`hashlib`). A new field means a new label (`/v2`): what is in flight
under `/v1` then stops verifying, which is acceptable for requests that
live seconds (§7).

## 4. What is bound, and where it is verified

| MAC | Inputs | Written by | Stored in | Verified by, before |
|---|---|---|---|---|
| The author's request | workspace, draft, send generation, author, inbox, content hash | `conversations.send_draft` (the authenticated author's call) | `drafts.request_mac` | `conversations.release_send`, before it writes anything (blob, state, event) |
| The stored send | workspace, send id, inbox, author, the outgoing document's blob (hash, size, type) | `conversations.release_send`, in the transaction that marks the draft `sending` | `drafts.send_mac` | the outbound-requests port (`stored_send`), before the connector records the send (`queue_send`) and again before every provider call (`send_message`) |
| A change request | workspace, request (event id), inbox, conversation, items | `conversation::request` / `request_each` (`publish_request`), in the member's request | `change_requests.mac` | the port (`changes_standing`), before `connectors.request_changes` records anything or asks Gmail for a stamp |
| An outbound change | workspace, row id, stream, request, provider message id, kind, labels added, labels removed | `connectors.request_changes`, for the rows it just inserted (by the ids it generated, never rows it found) | `outbound_changes.mac` | a stream's run (`outbound::backlog`), before it makes any change at the provider |

**The content hash** (`domain::binding::content_hash`) is SHA-256 of a
`Fields` (`campbooks/conversations.draft-content/v1`) over everything of the
draft that goes out: the inbox's address and name (the `From`), the kind,
the conversation and message it answers, To, Cc and Bcc (each address and
name), the subject, body and quoted history, `In-Reply-To`, `References`,
the thread key, and each forwarded attachment (id, file name, type, size,
and its blob's hash, size and type). Blobs are content-addressed and read
back verified against their hash, so binding the hash binds the bytes.

**Change items** are encoded as each typed item serialised (`serde_json`
of `RequestedItem`: field order of the type, so a stored copy that parses
back encodes the same).

**Clearing spent bindings.** `release_send` clears `request_mac` when it
stores `send_mac`; `cancel_send` clears it. A send again is a new
generation and a new binding.

## 5. On a mismatch

- `release_send`: `WARN a scheduled send is not bound to its author's
  request; it is not released` with the workspace and draft ids; the job
  completes (no retry), nothing is written (the check runs before the blob
  is stored), the draft keeps its state. A row that does not even decode is
  treated the same (it was not written by `send_draft`).
- The port, stored send: `WARN a stored send is not bound to the release
  that wrote it; it is not sent` (workspace, send id) and the send does not
  stand: `queue_send` records nothing (its own WARN), `send_message` settles
  it `unavailable` (#226 behaviour).
- The port, change request: `WARN a recorded change request is not bound to
  the request that made it; its changes are not made` (workspace, request
  id); no item stands, so `request_changes` records nothing and asks for no
  stamp.
- A stream's run: `WARN outbound changes not bound to the request they were
  recorded for; they are not made` (workspace, stream, a count); those rows
  end `skipped` (`unverified`), and their messages are marked to be
  re-emitted with the provider's state, so the conversation shows what the
  provider has.
- `send_draft` without a key: 500 and a WARN with ids; the draft stays a
  draft (the transaction rolls back).

No WARN carries an address, subject, body, label name or the tag.

## 6. Rows at deploy

A NULL binding never verifies. What exists when the migration runs:

- **Drafts `scheduled`** (scheduled by the previous version, in their undo
  window, no `request_mac`): the migration puts them back to `draft`
  (`send_at` cleared), the same state a release gives a draft whose author
  may no longer send it. Their authors see an editable draft and send it
  again; the old release jobs find nothing scheduled and do nothing. Chosen
  over re-signing: nothing in the database proves such a row authentic, and
  re-signing what SQL could have written is exactly the hole being closed.
- **Drafts `sending`** (released by the previous version, no `send_mac`):
  the port says they do not stand. `queue_send` records nothing, and
  `check_send` fails the draft (`unknown_outcome`) after its hour; one the
  previous version already queued is settled `unavailable` at its next
  attempt. Either way the draft can be sent again. An unclear attempt is
  still looked up first (#226).
- **Change requests and pending outbound changes** (no MAC): not made. The
  next sync brings the provider's state back.

The window is the deploy itself: a send waits seconds in its undo window,
and a change is made within seconds of the request. Run the API and the
worker of the same version.

## 7. Key rotation (noted, not built)

Rotating `CAMPBOOKS_SECRET_KEY` already re-keys everything derived from it
(sealed credentials, BFF sessions, cursors), so it is an operation of its
own. For bindings, the in-flight set is small and short-lived, so a rotation
can drain: requests in flight at the switch fail closed as in §6 (a
scheduled draft is left scheduled and its author cancels and sends again;
a `sending` one fails as unconfirmed; a pending change is not made). A
smoother rotation would hold a list of keys in `Binding` (sign with the
newest, verify against each, the tag's version byte or a key id choosing),
and a separate `CAMPBOOKS_SEND_BINDING_PREVIOUS_KEY` for the overlap. Not
built: no operator rotates the instance secret routinely today, and a
rotation of the instance secret breaks more than bindings.

## 8. What is still open

- **Replay of an authentic request.** Anything the database holds can be
  restored by SQL, including a binding. An attacker who read a draft's
  `request_mac` during its undo window could restore the draft exactly as
  the author scheduled it after the author cancelled, and plant a release:
  the binding verifies, and the mail the author cancelled goes out (same
  recipients, same content, from the same author). Likewise they could
  restore a sent draft and its binding to send it twice. The binding stops
  every send of content or recipients no member asked for; it cannot stop a
  re-play of one a member did ask for. Clearing the spent binding at release
  and on cancel narrows it to bindings read inside the undo window. Closing
  it fully needs state outside the database (a one-time token the release
  consumes outside Postgres), which this design does not add.
- **Rules.** A change made by a taught rule is signed by the server as it
  records it; a rule planted by SQL is a different question (the rule
  itself is not bound to the member who taught it).
- **`restore_to`** (where untrash puts a Zoho message back) is written by
  the connector after a trash and read by a later untrash; it is not bound.
  A tampered value can only move a message to another folder of the same
  mailbox in answer to a member's own untrash.

## 9. Migrations

The first free slots after `next`'s newest (`20261021220000`); no open PR
holds a later one:

- `20261021230000_conversations__send_bindings.sql`: `drafts.request_mac`,
  `drafts.send_mac`, `change_requests.mac` (nullable `bytea`, catalog-only),
  and the scheduled drafts back to `draft` (§6), with row-level security
  lifted for that one statement inside the migration's transaction.
- `20261021230100_connectors__outbound_change_mac.sql`:
  `outbound_changes.mac` (nullable `bytea`).

Both with a 10 s lock timeout.

### Rollback SQL

sqlx refuses to start with an applied migration it does not know. Before
redeploying an image older than this change, run once as the migration
login:

```sql
-- Roll back campbooks-cloud#227 BEFORE redeploying an older image.
BEGIN;
SET LOCAL lock_timeout = '10s';
ALTER TABLE connectors.outbound_changes DROP COLUMN mac;
ALTER TABLE conversations.change_requests DROP COLUMN mac;
ALTER TABLE conversations.drafts DROP COLUMN send_mac, DROP COLUMN request_mac;
DELETE FROM _sqlx_migrations WHERE version IN (20261021230000, 20261021230100);
COMMIT;
```

The drafts put back to `draft` stay drafts (their authors send them again).

## Prod risks

- **Background work that could stop.** Every release, every send attempt,
  every change request and every stream run that has pending changes now
  verifies a MAC (microseconds) and reads one more column by primary key. If
  the instance secret were missing or different between the API and the
  worker, **every send and every change at the provider would stop**:
  releases would leave drafts `scheduled` (WARN `a scheduled send is not
  bound…`), sends would fail `unavailable`, changes would end `skipped`
  (`unverified`). Inbound sync is not affected. The startup WARN and those
  WARN lines are what to watch; the fix is the same secret in both.
- **A legitimate send refused by a bug** in the encoding would stay
  `scheduled` (the author can cancel and send again), not vanish; the
  release tests cover send, undo and send again, failed and sent again, and
  every edit the binding must catch.
- **Deploy**: drafts in their undo window at the migration go back to
  drafts (§6).
