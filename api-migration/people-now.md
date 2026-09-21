# People & Now — `/api/app`

**Status:** ⬜ not started · **Priority:** P1 (People read + one action proves the pattern) · **Depends on:** [`00-auth.md`](00-auth.md) (bearer auth + `Api::App::BaseController`)

People is the inbox as a directory of who-needs-you: persons and organizations sorted by
`PeopleStanding` score, with Scout's lane labels and stand-note text. Now is the home feed
reframed as a decision queue: attention cluster + ranked timeline, the Now ledger, and Scout's
activity log. Both surfaces share the same underlying feed pipeline (`Feed::Reader`,
`Feed::Generator`, `Feed::RefreshJob`) and broadcast on per-user Turbo channels that become
JSON channels for the SPA.

## What the SPA must render

### People

- **Directory list** (`/people`): counterparts (persons + orgs) split into Need-you
  (standing `needs_you: true`, ranked by score) and Latest (all, by `last_activity_at`).
  Each row carries: name, subtitle, avatar initial / avatar email, verb label
  (`reply|decide|do|pay|chase|nudge`), Scout's stand line (`People::StandCopy.line`),
  wait days, unread flag, tags, and `needs_you` / `score`.
- **Streams tab** (`/people/streams`): inbox-group streams with thread summaries; each
  stream opens as a thread list.
- **Person page** (`/people/:id`): threads newest-first (paginated), newest thread open,
  older threads lazy-loaded. Scout's stand note (`People::Standing`, rendered as
  `People::StandCopy.note`). Person details rail (contact facts from `People::Profile`).
- **Org page** (`/people/orgs/:id`): people + services side by side, org stand note.
- **Details rail** (`/people/:id/details`): contact facts, sender kind, relationship,
  state, tags, linked documents/events, merge.
- **Inline actions** on any row: done / undo_done / snooze / unsnooze / star / unstar /
  archive / unarchive / paid. Each returns a refreshed row for optimistic update + undo.

### Now

- **Deck screen** (`/now`): segment rings (All / Priority / Follow-ups / Mail / Time),
  attention cluster + ranked timeline (paginated), `Now::Ledger` stats, `Now::Log`
  activity rows, inbox state (`Home::InboxState`).
- **Feed item actions** (`/feed/items/:id`): act / dismiss / seen / undo — each returns
  the updated item state and an undo token.
- **Log undo** (`/now/log/:id/undo`): reverse a Scout-logged action (archive→unarchive,
  tag→remove_tag) via `EmailActions`.
- **Activity feed** (`/activity`): workspace-wide retrospective domain events, paginated,
  read-only.

## Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `GET /people` | `people#index` | Directory list; Turbo Stream for infinite scroll |
| `GET /people/:id` | `people#show` | Person page; threads paginated |
| `GET /people/streams` | `people/streams#index` | Streams tab |
| `GET /people/streams/:name` | `people/streams#show` | Single stream thread list |
| `GET /people/orgs/:id` | `people/organizations#show` | Org detail page |
| `POST /people/:id/actions/:kind` | `people/actions#create` | done / undo_done / snooze / unsnooze / star / unstar / archive / unarchive / paid |
| `GET /people/:id/threads/:thread_id` | `people/threads#show` | Lazy thread load |
| `GET /people/:id/messages/:message_id` | `people/messages#show` | Lazy message body |
| `GET /people/:id/details` | `people/details#show` | Details rail |
| `PATCH /people/:id/details` (rename, relationship, kind, state) | `people/details#{rename,relationship,kind,state}` | Contact mutations |
| `POST /people/:id/details/analyze` | `people/details#analyze` | Re-run AI analysis |
| `POST /people/:id/details/attention` | `people/details#attention` | Mark for attention |
| `POST /people/:id/details/merge` | `people/details#merge` | Merge contacts |
| `GET /now` | `now#index` | Now deck |
| `POST /now/log/:id/undo` | `now#undo_log` | Undo a Scout log action |
| `POST /feed/items/:id/act` | `feed/items#act` | Act on a feed card |
| `POST /feed/items/:id/dismiss` | `feed/items#dismiss` | Dismiss a feed card |
| `POST /feed/items/:id/seen` | `feed/items#seen` | Mark seen |
| `POST /feed/items/:id/undo` | `feed/items#undo` | Undo last act |
| `GET /feed/items/:id/preview` | `feed/items#preview` | Email preview for a card |
| `GET /activity` | `activity#index` | Workspace activity feed, paginated |

## `/api/v1` coverage today

🟡 **Flat contacts only.** `GET /api/v1/contacts` and `GET /api/v1/contacts/:id`
(`Api::V1::ContactSerializer`) expose raw contact fields. There is **no** coverage for:
standings, lanes/verbs, stand lines/notes, directory ordering, the `PeopleStanding`
materialized table, feed items, the Now deck, the Now ledger/log, feed item actions,
streams, org pages, or the activity feed. All net-new.

## `/api/app` endpoints to build

### People — directory & list

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/people` | Directory: paginated `PeopleStanding` rows for the current user (Need-you first, then Latest). Accepts `?q=` (search), `?tab=latest\|needing`, `?page=`. Returns rows with stand line text and lane verb. | `PeopleStanding.for_user`, `People::StandCopy.line`, `People::Standings.refresh!` (background) |
| `GET  /api/app/people/streams` | Streams list with inbox-group summaries (thread count, last message). | `People::StreamsController` logic, `Emails::TagGroups` |
| `GET  /api/app/people/streams/:name` | Single stream: thread list paginated. | `People::StreamsController#show` |
| `POST /api/app/people/:id/action` | Row action: `{ kind: done\|undo_done\|snooze\|unsnooze\|star\|unstar\|archive\|unarchive\|paid }`. Returns refreshed row + undo token. | `People::ActionsController#create` logic, `People::Standings.refresh_counterpart!`, `EmailActions` |

### People — person & org

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/people/:id` | Person page: standing, stand note, first page of threads (newest-first), contact facts. Marks newest thread read. | `PeopleController#show`, `People::Standing.for_person`, `People::StandCopy.note`, `People::Profile.for`, `Emails::MarkThreadRead` |
| `GET  /api/app/people/:id/threads/:thread_id` | Lazy thread: messages for one thread. | `People::ThreadsController#show`, `People::ConversationThread` |
| `GET  /api/app/people/:id/messages/:message_id` | Lazy message body. | `People::MessagesController#show` |
| `GET  /api/app/people/orgs/:id` | Org page: standing, stand note, member persons + services, streams. | `People::OrganizationsController#show`, `People::Standing.for_organization`, `People::StandCopy.note` |

### People — details & mutations

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/people/:id/details` | Contact facts, sender kind, state, tags, linked docs/events. | `People::DetailsController#show`, `People::Profile` |
| `PATCH /api/app/people/:id/details` | Update name, relationship, kind, state. Body: `{ field, value }`. Returns updated details. | `People::DetailsController#{rename,relationship,kind,state}` logic |
| `POST /api/app/people/:id/details/analyze` | Trigger AI re-analysis of the contact. | `People::DetailsController#analyze` |
| `POST /api/app/people/:id/details/attention` | Toggle attention flag. | `People::DetailsController#attention` |
| `POST /api/app/people/:id/details/merge` | Merge into another person (`{ target_id }`). | `People::DetailsController#merge` |

### Now

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/now` | Deck: segment counts, attention pairs + first timeline page, ledger, log, inbox state. Accepts `?segment=all\|priority\|follow_ups\|mail\|time&page=`. | `NowController#index`, `Feed::Reader`, `Now::Ledger`, `Now::Log`, `Home::InboxState` |
| `POST /api/app/now/log/:id/undo` | Undo a Scout log action. Returns updated log row + undo confirmation. | `NowController#undo_log`, `EmailActions` |
| `POST /api/app/feed/items/:id/act` | Act on a feed card (archive, tag, hold ask, …). Returns updated item + undo token. | `Feed::ItemsController#act` |
| `POST /api/app/feed/items/:id/dismiss` | Dismiss a feed card. | `Feed::ItemsController#dismiss` |
| `POST /api/app/feed/items/:id/seen` | Mark seen (no undo). | `Feed::ItemsController#seen` |
| `POST /api/app/feed/items/:id/undo` | Undo last act on a card. | `Feed::ItemsController#undo` |
| `GET  /api/app/feed/items/:id/preview` | Email preview for a card (rendered HTML). | `Feed::ItemsController#preview` |
| `GET  /api/app/activity` | Workspace activity feed (domain Events), paginated. Read-only. | `ActivityController#index` |

## Read models / serializers needed

All are net-new POROs in `app/serializers/api/app/`, following the plain-PORO convention
from `/api/v1`:

- **`Api::App::PeopleStandingSerializer`** — one directory row: `id`, `counterpart_type`
  (Person / Organization), `name`, `subtitle`, `avatar_initial`, `avatar_email`,
  `needs_you`, `verb`, `stand_line` (via `People::StandCopy.line`), `wait_days`,
  `unread`, `score`, `last_activity_at`, `tags`, `email_message_id`,
  `feed_item_id`. Reads from a `PeopleStanding` row; no extra queries at list time.
- **`Api::App::PersonSerializer`** — full person screen: standing (from
  `People::Standing.for_person`), stand note (`People::StandCopy.note`), contact facts
  (from `People::Profile`), first page of threads. Wraps
  `Api::App::ConversationThreadSerializer` per thread.
- **`Api::App::OrgSerializer`** — org screen: standing, stand note, people rows, service
  rows.
- **`Api::App::ConversationThreadSerializer`** — thread header + messages list. Wraps
  `Api::App::MessageSerializer` per message. Backed by `People::ConversationThread`.
- **`Api::App::FeedItemSerializer`** — one deck card: `id`, `kind`, `score`, `attention`,
  `sort_at`, `subject_type`, `subject_id`, `subject` (inline summary from the source —
  see `Feed::Reader#present`), `snoozed_until`, `seen_at`, `undo_token` (optional). The
  full card content (email body, document link, etc.) is source-specific; keep it shallow
  in the list and let `preview` fetch the rich form.
- **`Api::App::NowDeckSerializer`** — deck screen payload: `segment`, `segment_counts`,
  `attention` (array of `FeedItemSerializer`), `timeline` (array + pagination meta),
  `ledger` (from `Now::Ledger#buckets`), `log` (from `Now::Log#events`), `inbox_state`
  (from `Home::InboxState#state`).
- **`Api::App::StreamSerializer`** — one stream row: name, icon, thread count, last
  message snippet.

## Real-time

The Hotwire broadcast channel names become the **JSON channel identifiers** the SPA
subscribes to (detailed in [`realtime.md`](realtime.md)):

- **`people_<user_id>`** — broadcasted by `People::Standings.broadcast_changes!`
  (in `app/services/people/standings.rb`) after every `refresh!` or
  `refresh_counterpart!`. Carries row replace / remove / new-pill events. The SPA maps
  these to TanStack Query cache patches on the `people` list key.
- **`now_<user_id>`** — broadcasted by `Feed::LiveDeck.broadcast` (in
  `app/services/feed/live_deck.rb`) when a new feed item arrives. The SPA appends the
  card to the deck without a full refetch. `Feed::RefreshJob` and
  `People::StandingsRefreshJob` are the upstream triggers.

Both channels are currently Turbo Streams (HTML). The JSON channel design — wrapping the
same events as `{ event, payload }` — is specified in `realtime.md`.

## Open questions

- **Stand line locale**: `People::StandCopy.line` and `.note` call `I18n.t(…)` using
  `Current.user`'s locale. In the API context `Current` is set the same way as the web
  app, so this should work — verify that `around_action :switch_locale` carries over to
  `Api::App::BaseController`.
- **Thread lazy-loading strategy**: the web app loads the first thread eagerly and
  subsequent threads via lazy Turbo Frames. The SPA can mirror this (first thread
  inline in `GET /people/:id`, rest via `GET /people/:id/threads/:thread_id`) or load
  all thread headers in one shot and bodies on expand. Decide before building to avoid
  rework.
- **Feed item "subject" depth**: `Feed::Reader#present` batch-loads subjects
  (`EmailMessage`, `Task`, `CalendarEvent`, etc.) to pass to card components. The
  serializer needs to decide how much of the subject to inline in the list vs. fetch
  on demand. Shallow (kind + id + title) in the list; full subject via the respective
  `/api/app/email_messages/:id` etc. is the safe default.
- **`People::Attention` vs. `PeopleStanding`**: the standing table already carries
  `verb`, `detail`, `detail_kind`, `data["ask"]` — the list serializer reads from the
  table directly (no re-projection). `People::Attention` is only needed when computing
  a fresh standing outside the table (e.g., in `People::Standing.for_person` for the
  person page). Confirm which path the person-page endpoint should use.
- **Streams endpoint auth**: streams are workspace-inbox-group scoped and require
  at least one readable email account. The gate is `Emails::TagGroups` — confirm the
  `/api/app` context sets `Current.workspace` before calling it (should follow from
  `Api::App::BaseController`).
- **`paid` action**: `people/actions#create` with `kind: paid` marks an invoice paid.
  This reaches `Money::Evidence` / document state changes. Verify the full path before
  exposing it at the same endpoint as the simpler row actions, as it may warrant its
  own endpoint or a `surface=money` param like the web controller uses.
