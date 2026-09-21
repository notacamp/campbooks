# Real-time layer — `/api/app`

**Status:** ❌ not started · **Priority:** Cross-cutting (P1 pilot: `people_<user_id>`) · **Depends on:** `00-auth.md`

The SPA cannot function without live updates — new mail appearing, notification counts
ticking up, Now feed cards appearing, people-lane standings refreshing. All of this works
today via Turbo Streams. The SPA needs **JSON over ActionCable** instead, feeding the
TanStack Query cache. This file documents what exists, why it cannot be reused as-is, what
to build, and how every broadcast site in the server must be adapted.

## `/api/v1` coverage today

❌ **None.** The public Doorkeeper API has no ActionCable channels and no real-time
endpoints. Everything here is net-new.

---

## How it works today

**All real-time is Turbo Streams broadcasting HTML fragments via `Turbo::StreamsChannel`.**
There are no custom ActionCable channels — `app/channels/` contains only `application_cable/`
(the connection base class and channel base class). Every live update is an HTML fragment
pushed by the server and spliced into the DOM by Turbo's `<turbo-stream>` element.

**Confirmed broadcast sites:** 21 files in `app/` call a `broadcast_*` method
(`broadcast_append_to`, `broadcast_prepend_to`, `broadcast_replace_to`, `broadcast_remove_to`,
`broadcast_refresh_to`).

**Confirmed subscription points:** 17 `turbo_stream_from` calls in `app/views` and
`app/components`.

**All broadcast origins are server-side — models, jobs, and services** — not controllers.
Key broadcast classes:

| Service / model | What it broadcasts |
|---|---|
| `Emails::InboxBroadcaster` | Thread-row insert/replace/remove for the inbox |
| `Emails::SkimTrayBroadcaster` | Email skim tray card updates |
| `Documents::SkimTrayBroadcaster` | Document skim tray card updates |
| `Feed::LiveDeck` | New Now-feed cards (append to `feed_timeline`) |
| `Notification` model (after_create_commit / after_update) | New notification appended, grouped update, dismissed removal, bell refresh |
| `EmailScanJob` | Sync-pill status (scanning started / finished) |
| `EmailChatReplyJob` / `ComposeChatReplyJob` / `AiSetupChatReplyJob` | Scout / compose / AI-setup chat reply fragments |

---

## Stream names in use

Every stream is scoped to a **user id** (or a record id for per-object streams). The full
set found in the codebase:

| Stream name pattern | Subscriber view | What it carries |
|---|---|---|
| `inbox_#{user.id}` | Every inbox view (all folders, groups, search) | Thread-row replace / remove (filter-safe, idempotent ops) |
| `inbox_feed_#{user.id}` | Default inbox only (unfiltered) | Thread-row prepend (new mail floated to top) |
| `now_#{user.id}` | Now surface (`feed_timeline`) | New feed-card appended |
| `people_#{user.id}` | People list | Standings row updates |
| `notifications_#{user.id}` | Notification center + bell | New notification appended, bell count refreshed, dismissed removed |
| `agent_chat_#{user.id}` | Scout chat overlay / email thread chat | Chat reply fragments |
| `compose_chat_#{user.id}` | Compose chat panel | Compose-chat reply fragments |
| `ai_setup_chat_#{user.id}` | AI setup wizard chat | Setup-chat reply fragments |
| `skim_#{user.id}` | Email skim tray | Skim-card updates |
| `doc_skim_user_#{user.id}` | Document skim tray | Doc-skim-card updates |
| `sync_status_#{user.id}` | Topbar sync pill | Sync-progress indicator HTML |
| `reconciliation_#{reconciliation.id}` | Money / reconciliation workbench | Workbench line updates |
| `@thread` (the email-thread object) | Email thread view (email chat) | Chat reply / auto-action fragments |

---

## Why the SPA can't use it

1. **HTML, not JSON.** Every broadcast pushes a rendered HTML partial. The React SPA has no
   DOM for Turbo to splice into, and has no use for server-rendered HTML — it renders from
   its own component tree driven by the TanStack Query cache.

2. **`Turbo::StreamsChannel` is coupled to the Turbo client.** It uses Turbo's proprietary
   wire protocol (`<turbo-stream action="append|replace|remove">` payloads). A React client
   cannot consume it without running a full Turbo.js stack alongside React, which defeats
   the migration.

3. **ActionCable connection auth uses the cookie session.** The current
   `ApplicationCable::Connection` (verify: `app/channels/application_cable/connection.rb`)
   identifies users via the cookie. Capacitor's system browser does not share cookies with
   the app's WebSocket — the SPA must use the bearer token from `Authorization: Bearer`
   (see `00-auth.md`). `Turbo::StreamsChannel` has no hook for this.

4. **Stream subscription is template-driven.** Views call `turbo_stream_from "stream_name"`
   in server-rendered templates. The SPA never loads those templates, so it can never
   subscribe via the current mechanism.

---

## What to build

**One generic `SyncChannel`** per user (or workspace) that broadcasts structured JSON
envelopes. The SPA subscribes once on login and routes messages to the TanStack Query cache
via `queryClient.setQueryData` / `queryClient.invalidateQueries`.

### Envelope shape

```json
{
  "channel": "people",
  "action": "upsert" | "remove" | "replace" | "append" | "refresh",
  "resource": "person_standing" | "notification" | "feed_card" | "email_thread" | "chat_message" | "sync_status" | ...,
  "id": "<record id or null>",
  "payload": { ... }
}
```

- `channel` maps to a stream name (e.g. `"people"` → was `people_#{user.id}`).
- `action` mirrors the Turbo action semantics but is cache-semantics not DOM semantics.
- `payload` is a serialized resource — the same shape the REST endpoint would return for
  that resource. This means **every broadcast site must call a serializer**, not render HTML.
- An `action: "refresh"` with no payload is a cache-bust signal: the client calls
  `invalidateQueries` and re-fetches from the REST endpoint.

### Two viable designs

**Option A — single `UserSyncChannel`** (recommended for P1):
- One channel subscription per authenticated user: `UserSyncChannel.subscribe(user_id)`.
- All surface envelopes (`people`, `notifications`, `now`, `inbox`, etc.) flow through it.
- Simpler subscription lifecycle; the client filters by `envelope.channel`.

**Option B — per-surface channels** (`PeopleChannel`, `NotificationsChannel`, etc.):
- Mirrors the current per-stream-name pattern.
- More granular — a surface that is not open does not receive its envelopes.
- More complex subscription management on the client.

**Recommendation:** start with Option A for the P1 People pilot. Migrate to Option B per
surface if message volume warrants it (people + notifications + inbox + now all on one
socket is fine for a single user; a workspace with shared mailboxes and many active users
may benefit from per-surface channels later).

### Server-side migration of each broadcast site

Every existing `Turbo::StreamsChannel.broadcast_*_to(stream, html: …)` call must gain a
parallel JSON broadcast:

```ruby
# Before (keep for Hotwire coexistence)
Turbo::StreamsChannel.broadcast_replace_to("people_#{user.id}", target: dom_id(standing), html: html)

# After (add alongside)
ActionCable.server.broadcast("sync:#{user.id}", {
  channel: "people",
  action: "upsert",
  resource: "person_standing",
  id: standing.id,
  payload: Api::App::PersonStandingSerializer.new(standing).as_json
})
```

During the coexistence period both broadcasts fire. Once a surface flips from Hotwire to
React, the Turbo broadcast for that stream can be dropped.

### Every `/api/app` mutation must also broadcast

Optimistic updates in the SPA assume the server confirms the mutation with the same
envelope shape the channel would deliver for a background change. Concretely:

```
PATCH /api/app/people/:id/archive → 200 { data: { … } }
  + ActionCable broadcast to sync:#{user.id} { channel: "people", action: "remove", id: … }
```

This lets the client's optimistic update resolve cleanly even when a background job (e.g.
`EmailProcessJob`) races with the user action.

---

## Channels to rebuild

| Current Turbo stream | New `UserSyncChannel` envelope channel | Primary consumer | Key resource type |
|---|---|---|---|
| `inbox_#{user.id}` | `"inbox"` | People inbox list (email threads) | `email_thread` row |
| `inbox_feed_#{user.id}` | `"inbox_feed"` | Default inbox (new mail inserts) | `email_thread` row |
| `now_#{user.id}` | `"now"` | Now surface feed timeline | `feed_card` |
| `people_#{user.id}` | `"people"` | People standings / lane rows | `person_standing` |
| `notifications_#{user.id}` | `"notifications"` | Notification center + bell count | `notification` |
| `agent_chat_#{user.id}` | `"scout_chat"` | Scout overlay chat | `chat_message` |
| `compose_chat_#{user.id}` | `"compose_chat"` | Compose chat panel | `chat_message` |
| `ai_setup_chat_#{user.id}` | `"ai_setup_chat"` | AI setup wizard | `chat_message` |
| `skim_#{user.id}` | `"email_skim"` | Email skim tray | `skim_card` |
| `doc_skim_user_#{user.id}` | `"doc_skim"` | Document skim tray | `doc_skim_card` |
| `sync_status_#{user.id}` | `"sync_status"` | Topbar sync pill | `sync_status` |
| `reconciliation_#{id}` | `"reconciliation"` (scoped to record id) | Money workbench | `reconciliation_line` |
| `@thread` (email thread object) | `"email_thread"` (scoped to thread id) | Email thread view | `chat_message` |

**P1 priority:** `people_#{user.id}` and `notifications_#{user.id}` — required for the
People pilot. `now_#{user.id}` follows as P2 when the Now surface flips.

---

## Auth

ActionCable connections must authenticate via the **session bearer token**, not a cookie,
for two reasons: (1) Capacitor's system-browser WebSocket does not share the cookie jar
with the app; (2) the SPA's cross-origin fetch already uses `Authorization: Bearer`.

**Implementation:**

```ruby
# app/channels/application_cable/connection.rb
module ApplicationCable
  class Connection < ActionCable::Connection::Base
    identified_by :current_user

    def connect
      self.current_user = find_verified_user
    end

    private

    def find_verified_user
      token = request.params[:token] ||
              request.headers["Authorization"]&.delete_prefix("Bearer ")
      session = Session.find_signed(token, purpose: :api_app)
      reject_unauthorized_connection unless session&.then { |s| !s.expired? }
      session.user
    end
  end
end
```

The SPA passes the bearer token as a query param on the WebSocket URL:
`wss://app.example.com/cable?token=<bearer>`. This is the same pattern used by
Capacitor-compatible Rails ActionCable setups; the token is stored in secure storage and
is the same token used for HTTP requests.

**Cross-ref:** `00-auth.md` → "The token model" for `Session#signed_id` and
`Session.find_signed`.

**Note:** the existing `ApplicationCable::Connection` likely uses `cookies.signed` or
`current_user` from a cookie session (verify: `app/channels/application_cable/connection.rb`).
That path must remain for Hotwire coexistence. The bearer-token path is additive — try the
bearer token first, fall back to cookie for the legacy Hotwire client.

---

## Read models / serializers needed

Each broadcast payload must be a serialized resource the SPA can cache directly. Add these
in `app/serializers/api/app/` alongside the per-surface serializers:

- `Api::App::SyncEnvelopeSerializer` — wraps any resource with `{ channel, action, resource, id, payload }`.
- `Api::App::NotificationSerializer` — also needed by `GET /api/app/notifications` (see `settings.md`).
- `Api::App::SyncStatusSerializer` — `{ scanning: bool, account_id, progress_text }`.
- Per-surface resource serializers are defined in each surface's tracker file (e.g.
  `PersonStandingSerializer` in `people-now.md`). The realtime layer reuses them — do not
  duplicate.

---

## Open questions

- **Coexistence period:** how long do both the HTML Turbo broadcast and the JSON
  `ActionCable` broadcast fire in parallel? The safest approach is: both fire until the
  surface flips to React, then the Turbo broadcast is removed. Confirm the team is
  comfortable with the extra broadcast overhead during coexistence.
- **Subscription grouping:** `UserSyncChannel` is one subscription per user. A user with
  two browser tabs open gets two subscriptions and each tab receives all envelopes; the
  React client must be idempotent on duplicate envelopes (TanStack Query `setQueryData`
  is already idempotent on the same data — this is fine by default).
- **Reconciliation stream:** `reconciliation_#{id}` is record-scoped, not user-scoped. The
  `UserSyncChannel` approach would need the client to filter by `id` field. Alternative:
  a short-lived `ReconciliationChannel` the Money surface subscribes to while the workbench
  is open (closer to Option B). Decide when Money surface is scoped.
- **Email thread chat stream:** `@thread` (the `EmailThread` object, passed as a
  broadcast target) is also record-scoped. Same decision as reconciliation above.
- **`sync_status` and the sync pill:** the sync pill today refreshes an entire HTML
  component. For JSON, the envelope just needs `{ scanning: bool, accounts: [...] }` so
  the React topbar re-renders the pill. Decide whether sync status belongs in the
  `UserSyncChannel` envelope or as a short poll (`GET /api/app/sync_status`).
- **Notification bell count:** today `broadcast_bell_refresh` replaces the entire bell
  component HTML. For the SPA, `me` bootstrap carries the initial unread count
  (see `00-auth.md`) and the `notifications` channel keeps it live with `{ unread_count: N }`
  in the envelope `payload`. Confirm this is sufficient or whether the SPA should poll `me`
  as a fallback.
