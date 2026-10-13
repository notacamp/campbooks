# Proposal

## Why

Campbooks Next holds no mail until something reads it from the provider.
ADR 0010 puts every outside source behind one module, `connectors`: it owns
connections, credentials and provider code, and turns each connection into
a **change stream** that the owning domain module applies. The
`conversations` module (#37) is being built as a pure consumer of such a
stream (`contracts/message-change-batch.v1.schema.json`); without a
producer, the inbox stays empty.

This change is the first slice of `connectors`: a member links a Google
account, Campbooks keeps its credentials encrypted, a sync engine reads it
adaptively and fairly, and the Gmail adapter produces MessageChangeBatch v1
documents for the member's inbox.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/36.

## What Changes

- **New module `connectors`** (`crates/modules/connectors`, schema
  `connectors`), scaffolded by `cargo xtask new-module connectors`.
  Language from `docs/domain-map.md`: Connection, capability, Adapter,
  Change stream, cursor, Outbound change, Health.
- **Connections.** A member connects a Google account with OAuth 2.0
  (authorization code + PKCE S256, offline access, `include_granted_scopes`
  for incremental consent). Starting returns the authorize URL; the web
  app's callback page completes it with an authenticated call by the same
  member in the same workspace, so a consent can never land in someone
  else's workspace. One connection per (workspace, provider, provider
  account); granting another capability later extends the same connection.
  Only `messages` is offered in this change; `calendar` and `files` are
  modelled but not requested yet.
- **Credentials encrypted at rest** in their own table, never returned by
  any operation: refresh and access tokens sealed with AES-256-GCM under a
  key derived from `CAMPBOOKS_SECRET_KEY`, bound to table, column and row.
- **Kernel: `kernel::crypto`.** The BFF's sealing helper moves to the kernel
  as `SecretKey` (loaded once from `CAMPBOOKS_SECRET_KEY[_FILE]`) and
  `Cipher::new(&key, purpose)` (HKDF-SHA256 with a purpose label). The BFF
  keeps its existing label, so sessions sealed before the move still open.
  PKCE, random tokens and token hashes move with it.
- **Health** per connection: `ok`, `needs_reauth`, `throttled`, `failing`,
  with a machine-readable reason; `connectors.connection_needs_reauth` when a
  grant is revoked or expired. Disconnecting revokes the grant at Google,
  deletes the credentials and stops the streams.
- **Sync engine.** One **stream** per connection and capability, with its
  cursor. A one-minute dispatcher enqueues `connectors.sync_stream` for
  streams that are due (`unique_key` per stream, so pushes, manual syncs and
  ticks collapse). Each run does one bounded page and reschedules: straight
  away while there is more to read, at the minimum interval after changes,
  doubling towards the maximum (hours) while idle. A per-provider budget
  (Postgres token bucket) caps request rates across every process; the
  kernel queue's per-workspace fair claiming plus one-page jobs give
  fairness. Throttling (429, `rateLimitExceeded`) pauses only that stream,
  for `Retry-After`.
- **Gmail adapter.** Initial window (default 90 days, newest first), then
  incremental `history.list` from the stored `historyId`; a `404` history
  id runs a bounded resync of the window. Messages are fetched with
  `format=raw`; the raw MIME, text part, HTML part and attachments are
  stored as blobs; Gmail labels map to mailbox state, read/starred flags,
  categories and user labels. Label-only changes are re-emitted from a
  per-stream message index without downloading the message again.
- **Producing batches.** Each page becomes one MessageChangeBatch v1 blob,
  kept attached to a `connectors.batches` row for a retention period, and
  `connectors.messages_changed` (`contracts/connectors.messages_changed.v1.schema.json`)
  is published in the transaction that advances the cursor. Contract tests
  validate every produced batch and event against the schema files
  (`boon`, as #37 does).
- **Inboxes through events, not tables.** When a connection gains the
  `messages` capability the module mints the inbox id and publishes
  `connectors.capability_granted` (ids, plus a blob holding the account's
  address and display name). `conversations` creates the inbox from it
  (a consumer it adds; see design).
- **Events** (v1, ids only): `connectors.connection_added`,
  `connectors.capability_granted`, `connectors.connection_needs_reauth`,
  `connectors.connection_removed`, `connectors.messages_changed`.
  **Consumes** `workspaces.workspace_deleting` (revoke and purge) and
  `workspaces.member_left` (a departing member's grants are revoked).
- **Operations** under `/api/v1/w/{workspace_id}/connections…` with
  `operationId`s and `x-campbooks`: start a connection, complete it, list,
  get, disconnect, sync now. Scopes `connectors:read`, `connectors:write`.
- **Configuration** (inert when unset): `CAMPBOOKS_GOOGLE__CLIENT_ID`,
  `CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE` (or `…__CLIENT_SECRET`),
  `CAMPBOOKS_CONNECTORS__*` for the window, poll intervals and the provider
  budget. Google needs `CAMPBOOKS_SECRET_KEY` (startup error otherwise).
- **Tests without Google:** a wiremock fake of Google's OAuth and Gmail
  endpoints driven by recorded fixtures (fictional mail in en, pt-PT, es-ES,
  fr) and a fixed clock.
- **Contracts:** `contracts/` (MessageChangeBatch v1, messages_changed v1)
  is carried unchanged from #37; this change adds the schemas of
  `connectors.capability_granted` v1 and its account document.

## Capabilities

### New Capabilities

- `connectors`: connections and their capabilities, encrypted credentials,
  health and re-authorisation, the sync engine (streams, cursors, adaptive
  scheduling, rate limits, fairness), the Gmail messages adapter, and the
  message change stream it produces.

### Modified Capabilities

None. (`kernel::crypto` is an additive kernel API; the BFF's behaviour is
unchanged.)

## Impact

- Adds a module approved by `docs/domain-map.md` (M0, *connectors*) and ADR
  0010. No cross-module crate dependency, so no `architecture.toml` entry;
  consumed events are read through mirror types, checked by `cargo xtask
  arch`.
- Kernel: `crypto` (moved from the server's BFF).
- New crate: `mail-parser` 0.11.9 (Apache-2.0 OR MIT) for MIME;
  `boon` 0.6.1 (MIT OR Apache-2.0) as a dev-dependency for contract tests.
- Self-hosters: nothing changes until they configure a Google OAuth client.
  Then they need `CAMPBOOKS_SECRET_KEY` (already required for web sign-in)
  and the redirect URI `<CAMPBOOKS_PUBLIC_URL>/connections/google/callback`.

## Non-goals

- **Push** (Gmail `users.watch` + Pub/Sub). The interface is designed (a
  push only enqueues the stream's sync, collapsed by its unique key) and
  the config is reserved, but it is not implemented here; polling with
  `history.list` is the default and the self-host fallback anyway.
- **Outbound changes** (read, star, archive, labels to Gmail). Designed
  here; implemented in a follow-up once `conversations.conversation_changed`
  is published on `next` with an explicit intent per change (see design).
- **The Connections screen** and the web callback page (UI; needs an
  approved prototype). This change ships the operations the page calls.
- Calendar, files and bank capabilities; Microsoft Graph, Zoho and IMAP
  adapters (M1).
- The `Credentials` kernel port: no other module needs provider access yet.
- A separate `sync` process role (ADR 0011); sync runs on the `sync` queue
  of today's worker.
- Sending mail and drafts.
