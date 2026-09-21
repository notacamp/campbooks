# Spec — Inbox pilot  (first running surface; was "People pilot")

The first React surface to ship: the unified **Inbox** as a thin, real slice that proves the
plumbing (auth, api client, tokens, the shell, optimistic **undo-in-place**). Surface design in
[`core-ux.md`](./core-ux.md) §Inbox; design system in [`../DESIGN.md`](../DESIGN.md).

Ownership: spec + review = `campbooks-b4`. Scaffold/build wiring = `campbooks-1a`.
`/api/app` = `campbooks-e1`. Implementation = a Sonnet builder; `campbooks-b4` reviews.

## Scope (in)

- The shell (Sidebar + Scout bar + router) with **Inbox** at `/inbox` active.
- **Inbox list**: rows = Avatar, sender + time, subject, and a plain-language **annotation**
  derived from the row's `verb`/`stand_line`. **Important / Everything** tabs. Loading
  skeletons, empty state, error+retry.
- **Archive with undo** (the one mutation): optimistic remove, a dock/inline confirm
  "Archived · <name> · Undo"; Undo calls the inverse and restores position.
- **Reading pane** (≥900px): sender header + Scout "what this is" (from `stand_line`) + a
  stubbed body + actions; below 900px it's a route/sheet. (Body/compose stubbed for the pilot.)
- A **"Skim the long tail"** entry over Everything is stubbed (wire the flow in the Today/Skim step).

## Scope (out)

Compose/reply, Books/Calendar/Today, real thread bodies, verb/standings-specific chrome. Keep it
generic — a message list + archive/undo. (This is intentionally the "generic list" 1a scoped.)

## Data — the built `/api/app` contract (AUTHORITATIVE; owned by campbooks-e1)

Client adapts to the server; **no server changes required for the pilot.**

`GET /api/app/people?q=<search>&tab=latest|needing&page=<n>` →
`{ data: [Row], meta: { page, per_page, total, total_pages } }` (page-based pagy).

Row (`Api::App::PeopleStandingSerializer`): `id`, `counterpart_type` ("Contact"|"Organization"),
`name`, `subtitle`, `avatar_initial`, `avatar_email`, `needs_you`, `verb`
(reply|decide|do|pay|chase|nudge), `stand_line`, `wait_days`, `unread`, `score`,
`last_activity_at`, `email_message_id`, `feed_item_id`, `standing_kind`.

Action: `POST /api/app/people/:id/action` `{ kind: "archive" }`; Undo = `{ kind: "unarchive" }`.
Realtime: e1 broadcasts row upserts/removes on `UserSyncChannel` topic `"people"` (bearer-authed
cable) — subscribe to keep the list live (optional for the first cut; invalidate the query on message).

**Client mapping** (row → Inbox UI): `name`, `subtitle`; **Important tab** = `needs_you == true`
(or top `score`); **Everything** = all. Annotation from `verb` + `stand_line` (reply→"Wants a
reply", pay→"Invoice · to pay", decide→"Decision", nudge/chase→"Waiting on you", else the
`stand_line`). Signal place from `verb` (reply/decide→people, pay→money, do→time,
chase/nudge→now). `unread`; `last_activity_at`→time; `avatar_initial`/`avatar_email`→`<Avatar>`;
`counterpart_type=="Organization"`→service (square) Avatar. Auth = bearer token (no cookies) →
exercises the native path.

## Client

TanStack Query: `useInfiniteQuery(['inbox', tab, q])` (page-based); `useMutation(archive)` with
`onMutate` optimistic removal + snapshot, `onError` rollback, Undo = the `unarchive` mutation.
Types from e1's serializer (typelizer) or a hand-kept `InboxRow` type.

## Definition of done

Renders at `/inbox` with real `/api/app` data in dark + light; Important/Everything tabs; archive
+ undo works optimistically; keyboard path; skeleton/empty/error states; verified 375px + desktop;
a Playwright smoke test (load → archive → undo); token lint clean. `campbooks-b4` reviews before merge.
