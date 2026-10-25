# Design

## Context

#315 added the `microsoft` provider: a connection's tokens, refreshed at the
mail tenant, and a messages stream keyed by the Graph user id that reads
nothing. The other adapters share one engine (`features/sync_stream`): a run
takes budget, reads one bounded page through its adapter, and in the job's
transaction applies index updates, stores and announces the batch, advances
the cursor and sets the next run. Gmail follows a change log (history);
Zoho and IMAP read folders. Graph has both: folders, and a change log per
folder (`messages/delta`).

## Data ownership

Everything is in the `connectors` schema.

- The stream's cursor (`connectors.streams.cursor`) holds the tracked
  folders, each with its own delta position (a `$skiptoken` while a round is
  under way, then a `$deltatoken`), the window of a pass, the pass
  generation, when the folders were listed, and the messages a run still has
  to look up. Only the tokens are kept, never Graph's links.
- The index (`connectors.messages`) keeps each message by its immutable
  Graph id, with tokens like Zoho's and IMAP's: the place it is in (`INBOX`,
  `STREAM`, `SENT`, `JUNK`, `TRASH`, `ARCHIVE`, `USER`), `folder:<id>` for a
  stream folder or a member's own, `UNREAD`, `STARRED` and `category:<name>`.
- New table `connectors.microsoft_subscriptions` (migration
  `20261106010000_connectors__microsoft_sync.sql`): one row per connection,
  with its workspace and stream (composite foreign keys
  `(connection_id, workspace_id)` and `(stream_id, connection_id)`, like
  `imap_idle_leases`), the Graph subscription id, the keyed hash of its
  `clientState` (HMAC-SHA256 under the instance key, `Signer::sign`), when it
  expires, and when to try subscribing again after a failure. Row-level
  security as every workspace table, plus one maintenance-scope policy:
  `FOR SELECT` only, so the callback can find a subscription's workspace.
  The row goes with its connection.

No event is added or changed: a notification only makes a stream's run due,
and the run announces `connectors.messages_changed` as every run does.

## The adapter

Per run, after the token and the budget:

1. **Folders.** At a pass's start and every six hours: the well-known
   folders by name (`inbox`, `sentitems`, `junkemail`, `deleteditems`,
   `archive`, and the never-read `drafts`, `outbox`, `conversationhistory`,
   `syncissues`), then the folder tree from `/me/mailFolders` and
   `childFolders`, at most `…__MAX_FOLDERS` (default 100). A folder's shown
   name is its path (`Campbooks/Updates`), so a stream folder is recognised
   by `marks::of_name` as IMAP's is. A tracked folder that disappears starts
   a resync.
2. **Delta.** One page per folder per run (`Prefer: odata.maxpagesize=50,
   IdType="ImmutableId"`). A folder without a position starts a round with
   `$filter=receivedDateTime ge <window start>`; a `@odata.nextLink`'s
   `$skiptoken` continues it at once, a `@odata.deltaLink`'s `$deltatoken`
   is where the next run starts. The URL is always rebuilt from the fixed
   Graph base, the folder id (a path segment) and the token.
3. **Items.** A new message is downloaded with `/me/messages/{id}/$value`
   (refused above `…__MAX_MESSAGE_BYTES` and recorded as oversized, no
   download started past `…__RUN_BUDGET_BYTES`) and stored by
   `mail::store_message`; a known one whose tokens changed is emitted again
   from its stored document without a download. Drafts are skipped. An item
   `@removed` from a folder is looked up (`/me/messages/{id}`, at most 20 per
   run, the rest kept in the cursor): gone is a removal, elsewhere is a move.
4. **Resync.** Graph forgetting a folder's delta state (410,
   `syncStateNotFound`, `resyncRequired`, `syncStateInvalid`) or a vanished
   folder starts a pass: every folder's position is dropped, the generation
   goes up, and when every folder has finished its round the indexed
   messages of the window the pass did not list are removed (Gmail's rule).
5. **Byte budget.** A folder whose page could not be finished within the
   budget keeps its position and the run continues at once, as Gmail's does.

Graph errors carry a status and a sanitised code only. 429, and 503 with
`Retry-After`, are `RateLimited` with the wait capped at an hour; the run's
trouble handling sets `throttled` and the next run, as for every provider.

## Change notifications

### Alternatives considered

| | Poll only | Notifications on whenever the public URL is `https` | Notifications opt-in (`…__NOTIFICATIONS`) |
|---|---|---|---|
| New mail latency | the poll interval | seconds | seconds where turned on |
| An instance behind a proxy that does not forward `/hooks/*` | fine | every subscribe fails Graph's validation | fine until turned on |
| Moving parts in production while the feature is new | none | subscriptions, callback | none until chosen |

**Chosen: opt-in**, and only with an `https` `CAMPBOOKS_PUBLIC_URL` whose
host is not `localhost`: otherwise the stream is polled, with a warning at
start when notifications were asked for. Nothing about notifications ever
stops the server.

### The subscription

A run of a Microsoft stream, before it reads, makes sure the connection has
a live subscription: none, or one expiring within a day, is created
(`POST /subscriptions`, `created,updated,deleted` on `me/messages`, about
three days, `clientState` = 256 random bits, lifecycle notifications to the
same URL) or renewed (`PATCH`; a 404 creates a new one). A failed create is
retried an hour later, and the stream is polled meanwhile. While the
subscription is live the stream's next poll is the longest poll interval:
`CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS`, so it stays within the poll bounds
whatever they are. Renewal happens in those runs, so a subscription is
renewed well before it expires whenever the longest interval is under two
days (it is 4 hours by default, 120 seconds in the cloud); if not, a lapsed
subscription is simply created again.

### The callback

`POST /hooks/microsoft/mail` is public and outside the operation catalogue
(no automation or assistant can call it). It answers 404 unless Microsoft
mail and notifications are on. With `validationToken`, it answers the token
as `text/plain` (at most 1024 characters) and does nothing else. Otherwise
it reads at most 64 KiB of JSON with at most 100 notifications, and:

1. looks the subscriptions up in the maintenance scope (`FOR SELECT` on
   `microsoft_subscriptions` only: ids, the hash and the workspace);
2. keeps the notifications whose `clientState` verifies against the stored
   hash (`Signer::verify`, constant time); the `clientState` is never logged;
3. for each connection kept, in its own workspace: when the stream is
   neither paused nor waiting for the member, makes its run due now
   (`schedule::run_now`, which collapses into a pending run: the debounce);
   a lifecycle `subscriptionRemoved` forgets the row, `reauthorizationRequired`
   makes it renew on that run;
4. answers 202, or 404 when no notification was kept.

The notification's resource and change type are not used: the run reads the
delta, so a forged-but-valid notification can only cause one early read.

## Scopes

The callback takes no OAuth scope (it is authenticated by the subscription's
`clientState`). No operation is added or changed.

## Tracked events and coverage

No tracked event is added or changed, and no operation, job, schedule or
handler is added: the callback is not an operation, and runs are the
existing `connectors.sync_stream`. No Coverage row changes.

## Security

- Fixed hosts (`graph.microsoft.com`, `login.microsoftonline.com`); folder
  and message ids are validated and URL-encoded as path segments; links in
  answers are read for their token only.
- No redirects, 15 s timeouts, answers capped (1 MiB for a listing, the
  shared message limit for MIME).
- The `clientState` is stored as a keyed hash and compared in constant time;
  unknown and wrong both answer 404.
- The callback does no Graph call and reads no mail; a valid notification
  makes one stream due, in its own workspace.
- Read-only against the mailbox.
