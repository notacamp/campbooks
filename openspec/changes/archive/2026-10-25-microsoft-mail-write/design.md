# Design

## Context

A member's change reaches a provider in two steps, the same for every
adapter (`provider-stream-labels`, #226, #242, `imap-write-back`):

1. `connectors.request_changes` (← `conversations.changes_requested`) maps
   each standing item to tokens (`outbound::map*`), records it in
   `connectors.outbound_changes` and binds its MAC to the stream, connection
   and mailbox. For a Microsoft stream it records `skipped` (`unsupported`)
   today (#315/#316).
2. The stream's next run takes the bound, pending changes
   (`outbound::backlog`), merges them with the shared plan
   (`domain/push.rs`), makes them through the adapter, records how each
   ended with the index's new state (`outbound::record_outcome`), and only
   then reads. The index holding what was written is what keeps the next
   read from echoing it.

A Microsoft message (#316) is in one folder, read or not, flagged or not,
with its Outlook categories; the index keeps that as tokens
(`microsoft/state.rs`): `folder:<id>`, the folder's role (`INBOX`, `STREAM`,
`SENT`, `JUNK`, `TRASH`, `ARCHIVE`, `USER`), `UNREAD`, `STARRED` and
`category:<name>`. Its ids are Graph's immutable ids. A send
(`connectors.send_message`) is claimed before every provider call and looked
up after an unclear one; Gmail and Zoho send through their APIs, IMAP over
SMTP from its verified address only (#313).

## Decisions

### 1. Where writes run: the stream's run, before its read

As Gmail, Zoho and IMAP: the Microsoft run takes the backlog, makes the
writes, records their outcome in its own transaction, then reads. Writes
and reads of one stream never interleave and share its budget, backoff and
health. A run makes the changes of at most **50 messages** (the longest
prefix of the backlog, in recorded order, that touches at most 50): the rest
are newer than the last one taken, so the run asks for another at once. Its
requests (four per message at most: a read, a patch, a re-read after a
conflict, a move) are charged to the connection's budget with the read's.

### 2. The plan's tokens and the index's

The plan works on the contract's tokens (`UNREAD`, `STARRED`, `INBOX`,
`TRASH`, `SPAM`, `mark:<stream>`), what `request_changes` records
(`outbound::map_microsoft`, the IMAP mapping: label changes are
`unsupported`). The index's tokens are seen through `state::view`: `INBOX`
for the Inbox or a stream folder, `TRASH` for Deleted Items, `SPAM` for Junk
Email, and the mark of the stream folder or stream category the message
has. After a write the index gets the tokens of the message as Graph now has
it (`state::tokens` of its new state), so the next delta compares equal and
emits nothing.

### 3. What each intent is at Graph

| Intent | Graph |
|---|---|
| read / unread | `PATCH /me/messages/{id}` `isRead` |
| star / unstar | `PATCH` `flag.flagStatus` `flagged` / `notFlagged` |
| archive | move from the Inbox or a stream folder to Archive |
| unarchive | move from Archive or a member's folder to the Inbox (the stream's folder for a stream move) |
| trash ("delete") | move to Deleted Items, remembering the folder it was in |
| untrash | move back to that folder while it is read and not a bin; else, for mail the inbox received, the Inbox; else not made (`not_movable`) |
| spam | move to Junk Email |
| not spam | for mail the inbox received, move to the Inbox; else not made |
| move to a stream | see 4 |
| add / remove label | `unsupported` (recorded skipped) |

Moves are `POST /me/messages/{id}/move` with `destinationId` a folder id
from the stream's cursor (never a name from a request). A folder the cursor
does not have (no Archive, a message now in a folder the reader does not
read) is never guessed: that part is not made (`not_movable`), the rest is.
Where a deleted message was is bound into the change's MAC exactly as Zoho's
and IMAP's (`restore_to`, #242), from the folder Graph says the message is
in when it is deleted, never from the index alone.

**Never a permanent delete.** The client has no `DELETE` of a message or a
folder, no `permanentDelete`, nothing that empties a folder; a message is
only ever patched or moved, and only one a pending, bound change names. The
fake Graph refuses and records any `DELETE`, and every write test checks it
saw none.

### 4. Streams in Outlook: the folder first, else a category

| | A stream move |
|---|---|
| Always a category | non-destructive, but a member's `Campbooks/Updates` folder (which #316 reads as the stream) would be ignored, and the message would show two streams |
| Always a folder | needs folders created in the member's mailbox |
| **The mailbox's own stream folder where it has one, else a category** (chosen) | the mapping #316 reads, both ways; nothing is created |

A move to a stream applies only to mail in the inbox (the Inbox or a stream
folder), as IMAP: to Primary, the message goes to the Inbox and loses every
stream category; to another stream, it goes to that stream's folder
(`Campbooks/<Stream>` under any parent, as the reader lists it) when the
mailbox has one, losing every stream category, and otherwise to the Inbox
with the category `Campbooks/<Stream>` (the stream mark's name) in place of
any other stream category. Other categories are kept. No folder is created,
and no master category: Graph sets any category name on a message, and the
master list needs `MailboxSettings.ReadWrite`, which is not asked for.

### 5. Conflicts: `changeKey`, then the provider's later change wins

| | Conflict check |
|---|---|
| Last write wins, no check (Zoho) | Graph has a version per message, so a later Outlook change would be lost silently |
| Compare the index only | the index may be a run behind Graph |
| **Read the message (`@odata.etag`, `changeKey`) and patch with `If-Match`** (chosen) | Graph's own optimistic concurrency, the etag pattern of the calendar writes |

For each message the push reads its state and `@odata.etag`
(`GET /me/messages/{id}?$select=…,changeKey`). When a token the change
touches differs between the index and Graph, and Graph's is not what the
member asks for, Graph changed it since the last read: the change ends
`superseded` (`changed_at_provider`) and the message is emitted again with
Graph's state, as an IMAP server's change after the folder was read
(`UNCHANGEDSINCE`). When Graph already has what the member asks for, nothing
is called and the change is made. Otherwise the patch carries
`If-Match: <etag>`; a 412 (or 409, `ErrorIrresolvableConflict`) re-reads the
message once and applies the rule again; a second one is Graph's change too.
A message with no etag is not patched (`refused`). Moves carry no
`If-Match` (Graph takes none); the read just before is their check.

### 6. Moves and the index

Messages are read and written with `Prefer: IdType="ImmutableId"`, so a move
keeps the message's id and its index row: the next read sees it leave one
folder and arrive in another, compares equal and emits nothing. A move whose
answer names another id (or cannot be read) keeps the index row with the new
place and puts the old id in the cursor's look-ups: the next read looks it
up, a removal when Graph no longer has it, and the new id is read as new.
No message is lost, none is duplicated.

### 7. Sending: sendMail with MIME

| | How a message goes out |
|---|---|
| `sendMail` with a JSON message | Graph builds the MIME and its own `Message-ID`; threading headers would be Graph's |
| A draft (`POST /me/messages`), then `send` | two calls, an item in Drafts when the second fails, which only a `DELETE` would clean |
| **`sendMail` with the RFC 5322 message, base64** (chosen) | one call, the message Gmail sends (`Message-ID`, `In-Reply-To`, `References`, Bcc in the headers, which Exchange strips on delivery), saved to Sent Items by Graph |

Campbooks never appends a copy to Sent Items: Graph files one. After a 202
the job looks the copy up in Sent Items by its `internetMessageId`
(`$filter`, at most two answers) and, when exactly one is there, indexes the
message as sent under that Graph id with the Sent Items tokens, announces it
and records the id on the send; the next sync knows the id and emits
nothing new. `connectors.send_completed` names the provider's id of every
sent message, so while the copy is not there yet the attempt stays unclear:
the next one (at least 90 seconds on) only looks it up (§8).

### 8. At most once

Every `sendMail` is preceded by an attempt committed on its own. A definite
answer (202, a 4xx, 429 or 503 with `Retry-After`) settles it; a timeout, a
5xx or a lost answer leaves it unclear. The next attempt first looks the
message up in Sent Items by its `internetMessageId`: found, it is sent;
not found or several, it is **never sent again** and ends `unknown_outcome`.
Graph saves to Sent Items asynchronously, so a lookup that finds nothing
does not prove the message did not go out; a duplicate to the recipients
is worse than an unknown outcome the member can check. 429 and 503 with
`Retry-After` retry the job after the wait (at most an hour).

### 9. From the verified mailbox only

| | Unverified Microsoft address |
|---|---|
| Send anyway (Gmail, Zoho) | Graph sends as the signed-in mailbox, but the address Campbooks holds is Graph's `mail`, which a tenant admin can set to an address in a domain nobody verified |
| **Refuse (`rejected`)** (chosen) | as IMAP: only an address someone proved is used as From |

`address_verified` is true only when the ID token's `xms_edov` matched
Graph's address (#315). A send from an unverified Microsoft connection ends
`rejected` before Graph is called (log code `address_not_verified`); the
operator adds `xms_edov` (deploy README), the member connects again. The
document's From must be the connection's address (case aside) and every
recipient an address; the MIME From is that address, and no `Sender` header
is set. Exchange itself refuses a From the mailbox may not send as.

### 10. Graph's 4 MB limit

Graph takes at most 4 MB in a request: the base64 MIME of a sendMail must
fit. A larger message (about 3 MB of attachments) fails `too_large` before
any call, as Gmail's 35 MB limit does; a 413 from Graph is `too_large` too.
Upload sessions need a draft at Microsoft (§7) and are left for later.

### 11. The reader

A Microsoft emission shows a member's change still waiting as they left it
(the pending tokens over its flags, as IMAP's reader), and a message whose
change ended refused, overtaken, not movable or given up (`reemit`) is
emitted again from the index with Graph's state, at most 20 a run, while the
stream is not in a pass.

### 12. HTTP

Every call goes to the fixed Graph base with validated ids as encoded path
segments, follows no redirect, and times out (15 seconds, a send a minute);
answers are bounded (64 KiB, a move's answer 1 MiB and then only its id is
read); errors carry a status and a sanitised code only. 429, or 503 with
`Retry-After`, is throttling, capped at an hour: a push stops and the
stream is `throttled` until then, a send waits and tries again.

### 13. Off by default

With `CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED` off the module has no Graph
client: `request_changes` records a Microsoft change `skipped`
(`unsupported`), the stream's run asks nothing of Microsoft, and a send ends
`not_connected`. No setting is added, so nothing new is checked at start.

## Data ownership and events

Schema `connectors` only, existing tables: `outbound_changes` (its kinds,
tokens, `restore_to` and MAC as for Zoho and IMAP), `messages` (index
tokens, `reemit`), `sends` (`provider_message_id`, `thread_key`), the
stream's cursor (`lookups`). No migration, so no new RLS, grant or key.

Events: none added. `connectors.messages_changed` and
`connectors.send_completed` carry Microsoft streams as any other.

## Endpoints and scopes

No endpoint is added or changed; OpenAPI is unchanged. The Graph scopes are
those #315 already asks for (`Mail.ReadWrite`, `Mail.Send`).

## AI

None.

## Tracked events

None added or changed: `connectors.outbound_changes_made`,
`connectors.send_succeeded` and `connectors.send_failed` already carry
`provider: microsoft`. No new operation, job, schedule or handler, so no new
Coverage row.

## Risks

- Exchange keeping the client's `Message-ID` on a MIME sendMail is what lets
  the Sent Items copy be found; were it not kept, every send would end
  `unknown_outcome` after its attempts (never sent twice). Worth checking
  against a real mailbox before the flag goes on.
- Sending needs `xms_edov` configured: without it no Microsoft send goes out.
