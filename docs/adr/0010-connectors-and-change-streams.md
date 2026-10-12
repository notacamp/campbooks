# 0010 — Every outside source is a connector producing a change stream

**Status:** Accepted · 2026-10-13

## Context
Campbooks reads and writes several providers per capability: mail (Gmail,
Microsoft Graph, Zoho, IMAP), calendars (Google, Graph, CalDAV), files
(uploads, Drive) and bank data (statement files today, bank connections
later). Coupling each domain module to its providers' SDKs means every new
provider touches domain code, and polling every account on a fixed interval
makes cost grow with the number of accounts rather than with activity.

## Decision
- A single **`connectors`** module owns connections, credentials and all
  provider code. Each **adapter** (provider × capability) turns a connection
  into a **change stream**: ordered, de-duplicated, normalised changes plus a
  **cursor** that records how far Campbooks has read.
- **Push first.** Adapters subscribe where the provider can notify (Gmail
  watch, Graph subscriptions, Google Calendar channels, IMAP IDLE). Polling is
  the fallback, scheduled adaptively per connection: it speeds up while a
  connection is active and backs off towards hours when it is idle.
- **Scheduling is fair.** Sync work runs on the durable queue with
  per-provider rate limits and per-workspace fairness, so one large account
  can't starve others and a throttling provider slows only its own work.
- **Domain modules consume, never fetch.** `conversations`, `schedule`,
  `documents` and `money` apply changes idempotently by provider id. To
  change something at the provider they emit an **outbound change**, which
  the adapter performs with retries and conflict checks (etags, UIDVALIDITY).
- **One consent, many capabilities.** A connection records which
  capabilities it grants; adding calendar to an existing Google connection is
  an incremental consent, not a new connection.
- Credentials never leave the module; other modules get short-lived access
  only through the `Credentials` port, and only for outbound changes the
  adapter can't perform itself.

## Consequences
- Adding a provider or a channel (chat, SMS) is an adapter, not a domain
  change; domain modules are testable against recorded change streams.
- Re-syncing is replaying a stream from an earlier cursor.
- The sync role can be scaled on its own (ADR 0011).
- Push needs a public callback URL; self-hosters without one fall back to
  adaptive polling automatically.

## Alternatives considered
- Provider code inside each domain module — duplicated auth and rate limiting,
  domain code tied to SDKs.
- Fixed-interval polling for every account — simple, but cost grows with
  accounts, not activity.
