# Design

## Context

Merged foundations: the durable event bus and job queue (ADR 0012) with
fair per-workspace claiming, unique and ordering keys; content-addressed
blob storage (`state.blobs`); the operation catalogue (ADR 0013); tenancy
with row-level security (`Workspace<S>`, `begin_for`); the web BFF, which
already seals tokens at rest (`crates/server/src/bff/crypto.rs`).

Being built in parallel: `conversations` (#37), the consumer of the mail
change stream. The ingest contract is fixed and published by #37 as JSON
Schema files in `contracts/` (MessageChangeBatch v1, the
`connectors.messages_changed` v1 payload). This module produces against
them unchanged; modules never import each other.

Owner decisions that bind this design (#37 review, 2026-10-13):
dependencies are constructor-injected into handlers (no service map on
`AppState`); event payloads carry ids and non-personal facts only, personal
data travels in blobs.

## Data ownership

Everything lives in schema `connectors`. Workspace-owned tables carry
`workspace_id` with `ENABLE` + `FORCE ROW LEVEL SECURITY` and the standard
policy; `campbooks_app` gets DML. Ids are UUIDv7 from Rust. Users and inboxes
of other modules are referenced by id only.

| Table | Holds |
| --- | --- |
| `connections` | id, provider (`google`), account_subject (the provider's stable account id), address (the account's email, lower-cased), display_name, granted_by (user id), capabilities (`text[]`, granted), scopes (granted, space-separated), health, health_reason, health_changed_at, created_at, updated_at. Unique `(workspace_id, provider, account_subject)`. |
| `credentials` | connection_id (PK), refresh_token (sealed `bytea`), access_token (sealed), access_expires_at, updated_at. Never selected by a read operation. |
| `authorizations` | A started consent: id, user_id, provider, capabilities, connection_id (when extending one), state_hash (SHA-256 of the `state` value), pkce_verifier (sealed), expires_at (10 minutes). Deleted when completed; expired rows pruned. |
| `streams` | One per connection × capability: id, connection_id, capability, inbox_id (minted here for `messages`), phase (`backfill`, `incremental`, `resync`), cursor (`jsonb`, adapter-specific: Gmail `{history_id, page_token, window_start}`), interval_seconds, last_synced_at, last_change_at, consecutive_failures, throttled_until. |
| `gmail_messages` | The adapter's index of what it has emitted: stream_id, provider_message_id, thread_id, label_ids (`text[]`), internal_date, document (blob ref: the normalised message without flags). PK `(stream_id, provider_message_id)`. |
| `gmail_labels` | Per stream: label_id, name, type. A cache of `labels.list` for user label names. |
| `batches` | id, stream_id, batch blob ref, cursor, created_at. The producer's hold on each batch blob (contracts/README.md); pruned after `CAMPBOOKS_CONNECTORS__BATCH_RETENTION_DAYS` (default 7). |

Two instance-level tables carry no personal data and no RLS, because
scheduling and rate limiting must see every workspace:

| Table | Holds |
| --- | --- |
| `sync_due` | stream_id (PK), workspace_id, due_at. The scheduling index the dispatcher reads; ids and times only. |
| `provider_budgets` | provider (PK), tokens, refilled_at. A token bucket per provider. |

### Blobs

Per message: the raw MIME (`message/rfc822`), the text part, the HTML part
and each attachment are `put` before the batch that references them (the
contract's rule). Each is attached to its `gmail_messages` row
(`connectors.gmail_message`) so the sweeper keeps it until conversations
attaches its own hold; content addressing makes the second hold free. The
batch document is attached to its `batches` row and released when the row
is pruned. Removing an index row detaches its blobs.

## Connections and consent

```text
member ──POST /connections/authorizations {provider, capabilities}──▶ api
api: row in authorizations (state hash, sealed PKCE verifier, user, workspace)
api ◀─ {authorize_url, expires_at}
browser → accounts.google.com (code_challenge S256, access_type=offline,
          include_granted_scopes=true, prompt=consent, state)
Google → browser → <public_url>/connections/google/callback?code&state   (web app page)
web app ──POST /connections/authorizations/complete {state, code}──▶ api (BFF session)
api: find by state hash; caller == authorization.user, workspace == path, not expired
     exchange code + verifier at Google's token endpoint
     read the account (id_token `sub`, `email`) and the granted scopes
     upsert connection, seal credentials, create streams for new capabilities
     publish connection_added / capability_granted; mark the streams due now
```

- **Binding.** The redirect lands on a web app page, and only an
  authenticated call from the member who started it, in the same workspace,
  can complete it. Someone who sends their own authorize URL to a victim
  cannot pull the victim's mailbox into their workspace: the victim's
  completion is refused (404, and the tokens are never exchanged). No extra
  cookie is needed, so native clients can use the same operation later.
- **Scopes requested:** `openid email` (account identity) plus the
  capability's scopes: `messages` → `https://www.googleapis.com/auth/gmail.modify`
  (read and change labels, no permanent delete; sending later needs no new
  consent). `calendar` and `files` map to their scopes when offered.
- **Incremental consent:** `include_granted_scopes=true`; adding a
  capability passes the connection id, and completion requires the same
  `sub`. A different account becomes a new connection.
- **Token refresh** happens before a job opens its transaction, under a
  per-connection advisory lock (concurrent runs refresh once and share the
  result); the new access token and expiry are sealed back. `invalid_grant` → health
  `needs_reauth` (reason `grant_revoked`), event, streams paused.
- **Disconnect** revokes at `oauth2.googleapis.com/revoke` (best effort,
  logged without the token), deletes credentials, streams, index rows and
  batch holds, publishes `connection_removed`. Allowed to the grantor and to
  owners/admins.
- **Verified address:** the account's email is used as given only when
  Google says `email_verified`; otherwise it is stored and announced with
  `address_verified = false`, and must not be treated as the account's
  identity.
- **Ownership:** a connection belongs to the **workspace** (its inbox is a
  workspace record) and records the **member who granted it**. The grant
  is personal: when that member leaves (`workspaces.member_left`), their
  connections are disconnected and revoked.

## Credentials and `kernel::crypto`

`kernel::crypto` provides `SecretKey` (32 bytes from
`CAMPBOOKS_SECRET_KEY_FILE` or `CAMPBOOKS_SECRET_KEY`, loaded once by the
server), `Cipher::new(&key, purpose: &'static [u8])` (HKDF-SHA256, AES-256-GCM,
fresh nonce, versioned format), and the helpers `random_token`, `hash`,
`same_hash`, `pkce_challenge`, moved verbatim with their tests. The BFF
builds its cipher with its current label (`campbooks bff token encryption
v1`), so nothing sealed before the move becomes unreadable. Connectors use
`campbooks connectors credentials v1`, and associated data
`connectors.credentials:<column>:<connection id>` (and
`connectors.authorizations:pkce_verifier:<id>`), so a sealed value cannot be
moved to another row, column or table. A changed secret key makes stored
credentials unreadable: the connection goes `needs_reauth` (reason
`credentials_unreadable`) instead of failing jobs.

## Sync engine

- **Streams and cursors.** A stream is the unit of scheduling, ordering and
  rate limiting. Its cursor is opaque outside the adapter.
- **Dispatch.** A schedule `connectors.dispatch_due` runs every minute and
  claims due rows from `sync_due` (`due_at <= now`, `FOR UPDATE SKIP LOCKED`,
  bounded), enqueuing `connectors.sync_stream { stream }` in each stream's
  workspace with `unique_key = stream` and `ordering_key = stream`. Bursts
  (a tick, "sync now", a future push) collapse into one pending job; a
  stream never runs twice at once.
- **One page per run.** A run refreshes the token if needed, takes budget,
  reads one bounded page from the provider (outside any transaction), stores
  blobs, then in the job's transaction updates the index, writes the batch
  row, publishes `messages_changed`, advances the cursor and sets the next
  `due_at`. A large mailbox is many short jobs, so the queue's per-workspace
  fair claiming interleaves workspaces.
- **Adaptive interval.** More to read → due now. A page with changes →
  `min_interval` (default 60 s). No changes → interval doubles up to
  `max_interval` (default 4 h). `connectors.sync_now` resets to the minimum
  and makes the stream due now; the client calls it when a member opens the
  inbox, which is what "fast when active" means until push lands.
- **Rate limits.** `provider_budgets` is a token bucket per provider
  (`CAMPBOOKS_CONNECTORS__GOOGLE__UNITS_PER_SECOND`, default within Gmail's
  per-project quota); a run takes the page's estimated quota units in one
  `UPDATE … RETURNING` and, if short, reschedules the stream a few seconds
  later instead of waiting. Per-account limits (Gmail: 250 units/user/s)
  are respected by one run per stream and bounded concurrency inside a page.
- **Failures.** The stream row is the schedule, not the queue's retry:
  provider trouble is recorded on it and the run ends normally. 401 → the
  access token is dropped and the stream runs again after a backoff,
  refreshing it (`invalid_grant` there → `needs_reauth`); after
  `FAILING_AFTER` refused runs in a row the connection is `needs_reauth`
  (`access_refused`). 429 or `rateLimitExceeded` →
  health `throttled`, `throttled_until` from `Retry-After` (else the
  minimum interval). 5xx and network errors → `consecutive_failures` grows
  and the next run backs off from the minimum, doubling per failure, up to
  the maximum interval; after `CAMPBOOKS_CONNECTORS__FAILING_AFTER` (default
  5) health is `failing`. Database and blob store errors retry the job. Any
  successful run returns health to `ok`.
- **Leases.** The dispatcher (and "sync now") pushes a stream's row
  [`LEASE`] (15 minutes) ahead when it enqueues a run, and the run sets the
  real next time when it ends. A run that dies leaves the leased row, which
  comes due again: the schedule never depends on a job surviving. A run
  with more to read enqueues its continuation directly, ordered after
  itself.
- **Push hook (designed, not built).** A push endpoint per provider only
  makes the stream due now (same unique key). For Gmail: `users.watch`
  renewed daily for connections when `CAMPBOOKS_GOOGLE__PUBSUB_TOPIC` is set,
  and `POST /api/v1/connectors/push/google` verifying the Pub/Sub OIDC token
  (audience + service account), looking the connection up by a hash of the
  address in the notification. Inert by default; polling stays as the
  fallback.

## Gmail adapter

- **Backfill.** At creation the stream records `history_id` from
  `users.getProfile` (so nothing that arrives during backfill is missed) and
  `window_start = now - backfill_days` (default 90). Each run lists one page
  of `messages.list` (`q=after:<epoch>`, `includeSpamTrash=true`, 50 ids,
  newest first), fetches unknown ids with `format=raw`, and stores the
  `pageToken`. When the list ends, the phase becomes `incremental`.
- **Incremental.** `history.list` from `history_id` (types `messageAdded`,
  `messageDeleted`, `labelAdded`, `labelRemoved`), up to 100 distinct message
  ids per run, last state wins. Unknown id → `format=raw`; known id with
  label changes → `format=minimal` for the current `labelIds`, re-emitted
  from the stored document with new flags (no raw download); deleted, or
  404 on fetch → `remove`. The cursor advances to the response's
  `historyId` when the pages are exhausted.
- **Resync.** `history.list` answering 404 (history expired) → phase
  `resync`: record a fresh `history_id`, re-list the window, emit upserts
  for unknown or changed ids, and removals for indexed ids in the window
  that are gone. Bounded by the window; older indexed messages are left
  alone.
- **Skipped:** drafts (`DRAFT`) and chats (`CHAT`).
- **Bounded memory:** new messages are downloaded at most five at a time and
  stored as each arrives (raw bytes dropped once their blobs exist), and a
  download is abandoned as soon as it grows past
  `CAMPBOOKS_CONNECTORS__GMAIL__MAX_MESSAGE_BYTES` (default 50 MiB): such a
  message is recorded in `connectors.gmail_oversized` and never fetched
  again. A run starts no download once it has fetched
  `…__RUN_BUDGET_BYTES` (default 64 MiB); the page is then incomplete, the
  cursor stays put, and the next run reads the same page or history again,
  skipping (window) or only re-checking (history) what is already indexed,
  so nothing is lost or emitted twice.
- **Normalising** (`format=raw` → MIME parsed with `mail-parser`):
  `INBOX` → `mailbox.in_inbox`; `SPAM`, `TRASH` → `in_spam`, `in_trash`;
  `UNREAD` absent → `read`; `STARRED` → `starred`; `SENT` without `INBOX`
  → `direction = outbound` (else inbound); `CATEGORY_UPDATES|PROMOTIONS|SOCIAL|FORUMS`
  → `categories`; user labels → `labels {key: label id, name}`;
  `threadId` → `thread_key`; Message-ID, In-Reply-To and References without
  angle brackets; the List-Id, List-Unsubscribe, Precedence and
  Auto-Submitted headers; `sent_at` from `Date`, falling back to
  `internalDate`.
- **Batch:** one MessageChangeBatch v1 per run, in provider order (oldest
  first within the page), `inbox` = the stream's inbox id.

## The contract with conversations

- **Produced:** MessageChangeBatch v1 blobs
  (`application/vnd.campbooks.message-changes+json; version=1`) and
  `connectors.messages_changed { workspace, inbox_id, batch, cursor }`,
  declared with `Background::publishes`, exactly as in `contracts/`. A
  contract test validates every batch and event the fake-Gmail tests
  produce against the schema files with `boon` 0.6.1.
- **Consumer wiring:** `cargo xtask arch` rejects a consumer of an event
  nobody publishes. Whichever of #36 and #37 merges **second** adds the
  one-line consumer in conversations, `conversations.ingest_messages_changed`
  → enqueue `apply_changes` ordered by inbox.
- **Inboxes:** conversations owns inboxes; this module never writes them.
  On a new `messages` capability it mints the inbox id (so batches can name
  it immediately) and publishes `connectors.capability_granted { workspace,
  connection, capability, granted_by, inbox_id, account: BlobRef }`, the
  account blob holding `{provider, address, display_name}`. New schema files
  `contracts/connectors.capability_granted.v1.schema.json` and
  `contracts/connection-account.v1.schema.json` describe both. Conversations
  consumes it to create the inbox with `granted_by` as `manager`
  (again added by whichever PR merges second). Because event deliveries are
  not ordered, `apply_changes` for an inbox that does not exist yet retries
  shortly instead of failing; the first batch is never published before the
  `capability_granted` event's transaction commits.
- **Removal:** `connectors.connection_removed { workspace, connection,
  inbox_ids }`; what happens to the inbox (keep read-only, archive) is the
  conversations module's call.

## Outbound changes (designed; follow-up)

- **Trigger:** `conversations.conversation_changed` with `origin = member`;
  `origin = provider` is never sent back.
- **Execution:** job `connectors.push_changes { stream, provider_message_ids,
  add, remove }` (provider label ids), enqueued with the stream's ordering
  key so it never interleaves with that stream's sync; one
  `messages.batchModify` per job; retries on 5xx/429 like sync.
- **Mapping:** read → remove `UNREAD`; unread → add `UNREAD`; star/unstar →
  `STARRED`; archive/unarchive → `INBOX`; labels → their Gmail label ids.
- **Conflicts:** changes are **set intents** (add/remove), idempotent, never
  toggles. Gmail offers no per-message precondition, so the provider is the
  source of truth: if someone changed the same flag in Gmail meanwhile,
  last write wins at Gmail and the next `history.list` brings Campbooks to
  the provider's state. Our own change comes back as a provider-origin
  upsert that conversations applies as a no-op (it publishes only on real
  change), so nothing loops. Ids Gmail no longer has (404) are dropped.
  Adapters with preconditions (Graph etags, IMAP `UIDVALIDITY`, calendar
  etags) re-read and re-apply the intent on a precondition failure.
- **Why not now:** the event is not on `next` yet (arch would reject the
  consumer), and today's `ConversationChanged` gives `changes` plus the
  resulting conversation `state`, which does not say which messages to mark
  unread or which label keys changed. The follow-up asks #37 for an
  additive, explicit intent (`set: {read?, starred?, archived?}`,
  `labels_added`/`labels_removed` keys) and a contract file for it.

## Events

Published (workspace-scoped, v1, ids and non-personal facts only):

| Event | Payload |
| --- | --- |
| `connectors.connection_added` | workspace, connection, provider, granted_by |
| `connectors.capability_granted` | workspace, connection, capability, granted_by, inbox_id (messages), account (blob ref) |
| `connectors.connection_needs_reauth` | workspace, connection, granted_by, reason |
| `connectors.connection_removed` | workspace, connection, inbox_ids |
| `connectors.messages_changed` | workspace, inbox_id, batch (blob ref), cursor |

Each of the first four declares its connection to the client change feed
(`connectors.connection`, upsert or delete, ids only), so clients refetch
it. Health moves between `ok`, `throttled` and `failing` are not announced
yet (follow-up).

Consumed (mirror types):

- `connectors.revoke_workspace_grants` ← `workspaces.workspace_deleting`:
  revoke every grant and delete every connection with what it holds at
  once (access must stop when the workspace is deleted, and nothing the
  module keeps is useful without the grant), plus started consents.
- `connectors.revoke_departed_grants` ← `workspaces.member_left`:
  disconnect (and revoke) the connections that member granted in that
  workspace.

## Endpoints and scopes

All under `/api/v1/w/{workspace_id}`; scopes `connectors:read`,
`connectors:write`. No operation ever returns a token.

| Operation | Endpoint | Scope | x-campbooks |
| --- | --- | --- | --- |
| `connectors.start_connection` | `POST /connections/authorizations` `{provider, capabilities, connection?}` → `{authorization, authorize_url, expires_at}` | write | command low; automation/assistant false |
| `connectors.complete_connection` | `POST /connections/authorizations/complete` `{state, code}` → connection | write | command low; automation/assistant false |
| `connectors.list_connections` | `GET /connections` | read | query, assistant |
| `connectors.get_connection` | `GET /connections/{connection_id}` (with streams' phase and last sync) | read | query, assistant |
| `connectors.disconnect` | `DELETE /connections/{connection_id}` | write | command high (stops sync, needs a new consent to undo); automation false |
| `connectors.sync_now` | `POST /connections/{connection_id}/sync` | write | command low; automation, assistant |

Visibility: a member sees the connections they granted; owners and admins
see all of the workspace's (health matters to them). Anything else is 404.
Client tokens are bound to one workspace (their token claim, then row-level
security) and see all of its connections, which is intended: they are the
workspace's own integrations. They cannot start, complete or disconnect one
(no member to bind the grant to: 403 `connection_needs_user`).

## Configuration

- `CAMPBOOKS_GOOGLE__CLIENT_ID`, `CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE`
  (or `CAMPBOOKS_GOOGLE__CLIENT_SECRET`): Google is offered only when both
  are set; starting a Google connection otherwise answers 409
  `provider_not_configured`. Set without `CAMPBOOKS_SECRET_KEY` → startup
  error naming the variable.
- `CAMPBOOKS_CONNECTORS__GMAIL__BACKFILL_DAYS` (90),
  `…__POLL__MIN_SECONDS` (60), `…__POLL__MAX_SECONDS` (14400),
  `…__GOOGLE__UNITS_PER_SECOND`, `…__FAILING_AFTER` (5),
  `…__BATCH_RETENTION_DAYS` (7).
- Reserved, inert: `CAMPBOOKS_GOOGLE__PUBSUB_TOPIC`.
- Google's endpoint URLs are constructor parameters of the adapter (the
  server passes Google's, tests pass the wiremock server's).
  `CAMPBOOKS_GOOGLE__BASE_URL` serves them all from a local fake Google, and
  is refused unless `CAMPBOOKS_ENV=development`.

## One seam for grants

Every way of obtaining a provider grant ends in
`domain::grants::create_or_refresh_connection(ProviderGrant)`: workspace,
member, provider, account subject, verified address, granted capabilities
and tokens in; the connection created or refreshed, credentials sealed,
events published and streams started out. `connectors.complete_connection`
calls it after its PKCE exchange; a sign-in flow that obtains the same
Google, Microsoft or Zoho grant can call it later without the connector's
start/callback.

The module is composed by the server with its dependencies
(`connectors::module(Deps { google: Option<Google>, cipher, clock, config })`,
read once at startup by `campbooks_server::Composition::from_env`; the
catalogues use the inert default). Handlers receive them as an axum
extension and jobs as fields of their registered handler (the
`JobHandler`/`EventConsumer` constructor injection #37 adds to the kernel,
taken here verbatim; whichever PR merges second takes the other's
version).

## Testing without Google

- A `wiremock` fake of `accounts.google.com`/`oauth2.googleapis.com` (token,
  revoke) and `gmail.googleapis.com` (profile, labels, messages list/get raw
  and minimal, history with pages and a 404, 429 with `Retry-After`, 401),
  serving recorded fixtures: fictional mailboxes at `example.com` in en,
  pt-PT, es-ES and fr, with threads, attachments, categories and labels.
- `FixedClock` for scheduling and expiry; `sqlx::test` databases; the
  in-memory blob store.
- Contract tests with `boon` against `contracts/*.json`.
- RLS tests for every workspace-owned table, as in workspaces.
- An `#[ignore]`d smoke test against a real Google client, documented for
  staging (#53).

## Alternatives considered

**Polling or push first.** (a) Push first (Gmail watch + Pub/Sub): lowest
latency, but it needs a public callback, a GCP topic and subscription and
OIDC verification of Google's pushes; most self-hosters have none, so
polling must exist and be good anyway. (b) **Adaptive polling with
`history.list` first, push as a designed hook (chosen).** An idle
connection costs one 2-unit call every few hours; activity and "sync now"
bring it to a minute. Push then only makes a stream due, a small follow-up.

**One generic engine or per-provider loops.** (a) A loop per provider
(the Rails app's minute-per-account scan): simple, but scheduling, rate
limiting, health and fairness are rewritten per provider and cost grows with
accounts. (b) **One engine of streams and cursors, adapters as plug-ins
(chosen):** an adapter implements "read one page from this cursor" and
"apply this outbound intent"; scheduling, budgets, health and batching are
shared, and Graph, IMAP and calendars become adapters.

**Scheduling: future-dated jobs or a due index.** (a) Each run enqueues its
own next run with `run_at`: no extra table, but every connection keeps a
future job in `kernel.jobs`, which ADR 0012 names as what slows its claim
scan, and a run that dies breaks the chain. (b) **A `sync_due` index and a
one-minute dispatcher (chosen):** one indexed query a minute regardless of
account count, the schedule is a row that can be inspected and repaired,
and the queue holds only work that is due. Costs one instance-level table
of ids and times.

**Raw MIME or parsed.** (a) Gmail's `format=full` (Gmail parses):
no MIME code here, but no faithful original (signatures, re-parsing after a
parser fix) and IMAP needs a parser anyway. (b) Raw only, consumers parse:
pushes MIME into every consumer. (c) **Raw as a blob plus parsed text, HTML
and attachments as blobs (chosen):** what the contract expects; content
addressing keeps duplicates free; one parser (`mail-parser`) for every
email adapter.

**Where encryption lives.** (a) Keep it in the BFF and copy it into
connectors: two copies of security code. (b) A `Credentials` service module:
premature, nobody else needs credentials yet. (c) **`kernel::crypto` with a
purpose label per user (chosen):** one reviewed implementation, independent
keys per purpose from one instance secret, the BFF's ciphertexts unchanged.
Per-workspace data keys (envelope encryption) are a later option behind the
same API.

**How the OAuth callback is bound.** (a) A server-side callback that
completes immediately: simplest, but the BFF cookie is `SameSite=Strict`
and is not sent on Google's redirect, so the server cannot tell who is
completing; an attacker's authorize URL could link a victim's mailbox into
the attacker's workspace. (b) A dedicated `SameSite=Lax` binding cookie:
works for the web, not for native clients, and is a second cookie scheme.
(c) **The redirect lands on the web app, which completes with an
authenticated call by the initiating member (chosen):** binding by
identity, no new cookie, works for native clients, and no GET endpoint
mutates (ADR 0013's `mutates` rule).

**How outbound conflicts are detected.** (a) Version vectors per message:
Gmail has no preconditions to honour them. (b) Read-before-write: still
racy, double quota. (c) **Idempotent set intents, provider as source of
truth, convergence on the next pull (chosen for Gmail)**, with real
preconditions where providers offer them.

**Member- or workspace-owned connections.** (a) Member-owned: the inbox
would vanish from colleagues when its grantor is away. (b) Workspace-owned
with no grantor: nobody accountable, and a departing member's mailbox stays
connected. (c) **Workspace-owned, grantor recorded, revoked when the
grantor leaves (chosen).**

**Copied from the Rails app on `main`?** Only on the merits: one consent
covering several capabilities, requesting offline access with a refresh
token, and treating `invalid_grant` as "reconnect". Not copied: a
minute-per-account poll, per-provider scan jobs inside the mail domain,
push verified by a shared secret in the URL, and storing credentials with a
framework's attribute encryption.

## Risks / Trade-offs

- **Google verification.** `gmail.modify` is a restricted scope; until the
  OAuth client is verified (#53), only test users can connect.
- **Index size.** One `gmail_messages` row per synced message. Bounded by the
  window plus new mail; the documents are blobs.
- **Contract coordination.** `capability_granted` and the consumer wiring
  depend on #37 adding two small consumers; the order of merges is
  documented above.
- **Revocation is per Google grant**, not per Campbooks connection: the
  same Google account connected in two workspaces of one instance shares a
  grant, so disconnecting it in one sends the other to `needs_reauth`. A
  second workspace is rare for one account; the member reconnects there.
- **Secret key rotation** makes stored credentials unreadable (reconnect),
  as it ends web sessions today. Key rotation with re-sealing is a
  follow-up.
- **The event log is never pruned**: payloads here carry no personal data,
  by design (#65).
