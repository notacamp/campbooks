# `/api/app` — first-party API migration tracker

Campbooks is moving off Rails/Hotwire to a decoupled **React + shadcn PWA** (Capacitor
for mobile) talking to Rails as a **pure JSON API**. This directory tracks the
**new first-party API (`/api/app`)** we must build, **surface by surface**, so the SPA can
replace each Hotwire screen. Mirrors the `ui-refactor/` builder-tracker convention:
each file catalogs one domain; builders read from it and mark items done.

> Two API surfaces exist by design:
> - **`/api/v1`** — the public **Doorkeeper** REST API for third parties. **Untouched.**
> - **`/api/app`** — this one. First-party, screen-shaped, unversioned, **reuses the same
>   services** (`People::Standings`, `Money::Page`, `Time::Agenda`, `EmailActions`,
>   `Emails::Sender`, …). Does **not** exist yet — everything here is net-new.

## Auth model — token-first (decided 2026-09-20)

**Bearer tokens, not cookies.** The web SPA and the Capacitor app both authenticate with an
opaque bearer token in `Authorization: Bearer <token>`. This is the cross-origin-friendly
path Capacitor needs (system browser + secure storage), and it lets one identity resolver
serve web and native identically.

**Design: session-backed bearer.** Reuse the existing `Session` model (and its MFA, audit,
inactivity-expiry machinery) rather than inventing a parallel token store or leaning on
Doorkeeper (which is third-party / consent-shaped). A login mints a `Session` and returns its
**opaque token**; `/api/app` resolves `Bearer <token>` → `Session` → `Current.user` /
`Current.workspace`. Full spec + the required `sessions` migration in [`00-auth.md`](00-auth.md).

Why not the alternatives:
- **Cookie session** — rejected: breaks cross-origin Capacitor; couples to same-origin Caddy.
- **Doorkeeper authorization_code/PKCE** — heavier, browser-consent shaped, meant for *other
  people's* apps calling us; overkill for our own first-party client.

The identity resolver is modeled on `Api::V1::BaseController#establish_acting_identity!`
(`app/controllers/api/v1/base_controller.rb`) but resolves a **Session token**, not a
Doorkeeper token. It must **fail closed** (401) the same way.

## Status legend

| Mark | Meaning |
|------|---------|
| ✅ | `/api/v1` already covers this resource — reuse the serializer/service, re-expose under `/api/app` with session auth |
| 🟡 | Partial — a raw resource exists in `/api/v1` but the **screen-shaped read model** (aggregation) does not |
| ❌ | No JSON coverage at all — build from scratch |

## Coverage summary

| Surface | Today | Tracker | Build size |
|---|---|---|---|
| **Auth / registration / 2FA / onboarding** | ❌ | [`00-auth.md`](00-auth.md) | **P0 blocker** |
| **People** (standings/lanes/stand-note) | 🟡 flat contacts | [`people-now.md`](people-now.md) | Large |
| **Now** (feed, deck, Scout log) | ❌ | [`people-now.md`](people-now.md) | Large |
| **Time** (merged agenda, focus blocks) | 🟡 raw asks/events | [`time-money.md`](time-money.md) | Medium |
| **Money** (evidence, reconciliation, loans) | ❌ | [`time-money.md`](time-money.md) | Large |
| **Paper / Files / Skim / Search** | 🟡 raw documents | [`paper-files.md`](paper-files.md) | Large |
| **Compose + email accounts** | 🟡 send/drafts | [`compose-email.md`](compose-email.md) | Medium |
| **Scout** (overlay, tools, chat) | 🟡 async chat | [`scout.md`](scout.md) | Medium |
| **Calendar** (views, accounts, event types) | 🟡 raw events | [`calendar.md`](calendar.md) | Medium |
| **Settings + integrations + notifications** | ❌ | [`settings.md`](settings.md) | Large (~30 ctrls) |
| **Real-time** (JSON channels) | ❌ HTML-only | [`realtime.md`](realtime.md) | Cross-cutting |

## The through-line

`/api/v1` is a **component library** for `/api/app`, not a substitute. It gives good
resource-level CRUD + serializers for ~10 domains (emails, documents, contacts, calendar
events, reminders, tasks/asks, drafts, scheduled emails, email templates, workflows, Scout
chat) — reuse those. It provides **zero** screen-shaped reads for the five places, and
**nothing** for auth, settings, notifications, files, skim, Money, compose-AI, or real-time.
Net: **~90% new endpoints.**

## Sequencing

1. **P0 — [`00-auth.md`](00-auth.md).** Session-backed bearer + `me` + registration + 2FA.
   Nothing renders without login.
2. **P1 — People pilot** ([`people-now.md`](people-now.md)): `people#index` read model + one
   action (archive w/ undo) + the `people_<user_id>` JSON channel
   ([`realtime.md`](realtime.md)). Proves the read-model + real-time + optimistic-update
   pattern end to end.
3. **P2+** — Now, Time, Money, Paper/Files, Compose, Scout, Calendar, Settings, in whatever
   order the product wants surfaces to flip from Hotwire to React.

## Conventions for every `/api/app` endpoint

- Inherits a new `Api::App::BaseController` (session-bearer auth; JSON envelope
  `{ data }` / `{ data, meta }` / `{ error: { code, message } }`; the **404-not-403** leak
  rule, same as `/api/v1`).
- **Screen-shaped:** an endpoint returns exactly what a screen needs, in one round trip —
  not a normalized resource the client must re-assemble. Aggregations live in the existing
  services; the controller is thin.
- **Reuses services, never re-implements** the read models or the mutations. If a mutation
  lives in a service (`EmailActions`, `Reconciliations::LineActions`, `Asks::HandOff`, …),
  call it; if it only lives in a web controller, extract it to a service first.
- New serializers are plain POROs in `app/serializers/api/app/` (mirrors `app/serializers/api/v1/`).
- Every mutation returns enough to drive **optimistic update + undo** (the client's TanStack
  Query cache patch), per the "instant feel" goal.
