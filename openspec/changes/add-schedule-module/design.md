# Design

## Context

- No calendar exists on `next`. `connectors` models `Capability::Calendar`
  ("not offered yet") and already extends a connection with a capability it
  lacks (incremental consent through the identity front door). Sync is
  adaptive polling (`domain/schedule.rs`, the sync dispatcher — unrelated to
  this module despite the name).
- `conversations.decide_message` asks Jev eight typed questions in one call
  per inbound message. `has_deadline` exists but only gates needs.
- `conversations.summarize_need` asks `complete` (GLM-5.3 Flash in prod, a
  reasoning model) for a summary plus `lapses_at`/`due_on`, only for messages
  that raise a need, and vetoes dates that don't parse or lie outside
  `[sent_at, sent_at + 366 d]`.
- ADR 0018: no code decides meaning from subject, body or attachment names.
  ADR 0009: extraction is `complete`'s job; `decide` may gate it.
- `tasks` (campbooks-cloud#43) is being built in parallel. Agreed seams are
  under *Seams with tasks*.

## Three layers, cheapest first

The owner's rules: Jev first, a conversational model only when Jev cannot do
the job, and **at most one `complete` read per email, covering everything
in it**. Jev answers typed questions with probabilities; it cannot write a
date (ADR 0009). So each layer answers what it can and passes on only what
is left.

| Layer | What it reads | Cost | What it gives |
| --- | --- | --- | --- |
| **L0 · structured data** | `text/calendar` parts (VEVENT, METHOD REQUEST/CANCEL/PUBLISH) and schema.org markup (JSON-LD or microdata) in the HTML | none | exact typed items |
| **L1 · Jev** | the decide pass's existing state | two questions in a call that already happens | is there a dated thing, and of which kind |
| **L2 · the message read (`complete`)** | the message's own words, bounded | the call that already runs for a need, or one call when the member first opens a flagged email | dated items, asks, the need summary |

### L0 — structured data (ADR 0020)

- **iCalendar**: the module parses each `text/calendar` part with an
  iCalendar parser (not regex). VEVENT → item; `METHOD:CANCEL` cancels the
  item with the same `UID`; `SEQUENCE` orders updates. Kind: `meeting` when
  the VEVENT has attendees, else `appointment`. When the member's synced
  calendar already holds that `UID` (the provider put the invite there
  itself), schedule links the item to that event instead of adding one.
- **schema.org**: JSON-LD `<script type="application/ld+json">` and
  microdata, the types the big senders already emit for Gmail:
  `FlightReservation`, `TrainReservation`, `BusReservation` → `travel`;
  `LodgingReservation` → `stay`; `EventReservation` → `show`;
  `FoodEstablishmentReservation` → `reservation`; `RentalCarReservation` →
  `travel`; `ParcelDelivery` → `delivery`; `Invoice` with
  `paymentDueDate` → an ask with verb `pay`, its due and
  `totalPaymentDue` as the amount. Unknown types are ignored.
- **Only for authenticated senders**: markup is trivially forged, so L0
  applies only when the receiving provider's authentication results show an
  aligned DMARC pass (the strict reading already specified in
  `ai-decide-facts` → *Authentication results are read strictly*), and Jev's
  `suspicious` answer is not accepted. Otherwise L0 abstains and the message
  goes on to L1/L2 like any other.
- **Why this does not break ADR 0018**: a sender's iCalendar part and
  schema.org block are data structured *for machines* by the sender, with
  typed fields. Reading them decides nothing about what prose means. ADR
  0020 records the carve-out and its limits (authenticated senders, typed
  fields only, never the prose around them); `ai-decide-facts` → *No content
  rules* names it.

### L1 — Jev, in the existing decide pass

Two questions join `triage_questions()`:

- `has_dated_thing` (noul): *"Does the email tell the recipient about
  something of theirs on a specific date or time: an appointment, a booking,
  a trip, a ticketed event, a delivery, a charge to their account, something
  expiring, or a date by which they must act? Sales and offers ending,
  newsletters, and dates only in quoted older messages do not count."*
- `dated_kind` (choice): `deadline` (an obligation: it becomes an ask) · `charge` · `appointment` · `meeting` ·
  `travel` · `stay` · `show` · `reservation` · `delivery` · `expiry` ·
  `none`, each with a one-line description in the question (e.g. `charge`:
  "money will be taken automatically on a date: a renewal, a subscription,
  a direct debit, a trial ending"; `deadline`: "the recipient must do
  something by a date, such as pay, sign, submit, reply or renew").

Both are recorded like every answer and, like every question the eval
report does not measure, never act on their own. They do two things:

- **Flag** a message: `has_dated_thing`, or the existing `asks_action` or
  `has_deadline`, at or above `CAMPBOOKS_CONVERSATIONS__DATED__FLAG`
  (default 0.6), the stream not accepted as `newsletters`, `suspicious` not
  accepted. Only a flag lets the read (L2) run; see *When it runs*. A
  `has_dated_thing` flag on a message L0 did not cover also tells the row
  and the reader "Scout can read this for dates". A flag costs nothing.
- **Name the kind** for the fallback when no `complete` role exists, and as a
  hint to L2 (not a constraint: one email may hold a flight and a hotel).

The threshold is a starting point, not a measured operating point. The
`decide-eval` set gains labelled dated-thing items written by the owner or
taken from members' corrections (never written by the agent that builds
this; see *Risks*).

### L2 — the message read: one `complete` call per email, for everything

`conversations.summarize_need` becomes **`conversations.read_message`**, and
`conversations.need_summaries` becomes `conversations.message_reads`. It is
the only `complete` call that reads an email's content (Scout's reply
drafts excepted: those write, on request). No module adds another:
`tasks` and `schedule` consume its result.

- **When it runs** (a job, never on a request path, once per message plus one
  retry after a failure). Every email gets **one** Jev pass (the existing
  call, all questions in it) and **at most one** read. The read runs only
  when the pass **flagged** the message (`asks_action`, `has_deadline` or
  `has_dated_thing` at or above its flag threshold, stream not
  `newsletters`, not `suspicious`) **and** either:
  1. a need opened for it (Reply, Do, Pay, Decide); or
  2. a member opened it: the open queues the job (`POST …/read`) and the
     reader shows "Scout is reading this…" until the result arrives on the
     change feed.
  A need with no flag (a plain Reply) gets **no read on arrival**: its
  summary comes from the read on first open, and until then Home shows the
  need's Jev-level line. This changes today's `summarize_need`, which reads
  every need's email: listed in `openspec/decisions-needed.md` with the
  recommendation to accept it. Nothing else queues a read: no backfill
  over old mail, no read of unflagged or unopened mail, and no Jev call
  outside the pass.
- **L0 first**: a message L0 fully covered (it found items and Jev's kind is
  among them) is not read for dates; a need on it still gets its summary
  read, with the L0 items given to the model as known items so it adds
  only what is missing.
- **Input**: sender, `sent_at`, the time zone of the inbox's personal owner
  (through a kernel port `TimeZones` that `schedule` implements; UTC for a
  shared inbox or until set),
  subject, own words without quoted history, bounded like the summary
  today; Jev's `dated_kind` as a hint; the conversation's known items (id,
  kind, title, when) so a newer email can `update` or `cancel` one instead of
  adding a duplicate; the open tasks whose source is this conversation or
  this sender's address within the same inbox, whoever holds them (most recent first, at most 10:
  `task_ref`, title, due), through a kernel port
  `OpenTasks::open_tasks(workspace, inbox_id, conversation_id, sender_address)` that
  `tasks` implements (an empty list until it does), so an ask can say it
  is the same obligation as one of them. Titles reach the model in-process
  only, never an event.
- **Output** (one strict JSON schema):
  - `summary` in `en`, `pt-PT`, `es-ES`, `fr`, `lapses_at` and `due_on`, as
    today, only asked for when a need is open; `due_on` is now the date of a
    `reply`, `review` or `waiting` need only (for `waiting`, the awaited
    person's date, as today);
  - `items[]` (at most 5), **things that happen at a time only**: `kind`
    (`charge` · `appointment` · `meeting` · `travel` · `stay` · `show` ·
    `reservation` · `delivery` · `expiry`), `title` (≤ 80 chars, e.g. "Flight LIS →
    AMS", never the subject), `starts` (RFC 3339 with offset, or a date when
    all-day), `ends`, `all_day`, `time_zone` (IANA), `place` (≤ 120),
    `quote` (the sentence that gives the date, verbatim), `updates` or
    `cancels` (a known item's id);
  - `asks[]` (at most 3), **every obligation of the reader**, dated or not
    (pay by, submit by, sign, renew by): `title` (≤ 80), `quote`, `due`
    (date or date-time, optional), `verb` (`do` | `pay`), `amount`
    (`{minor, currency}`, `pay` only, optional), `same_as` (a `task_ref`
    from the list given, optional). These are for `tasks`; a `do` or `pay`
    need takes the due of the read's first ask with its verb instead of
    `due_on`, until `tasks` owns those needs. An obligation is never
    also an item, so one read never says the same thing twice.
- **Instruction rules** (in the prompt, so code never guesses): only asks of
  the reader, never of other recipients or of the sender themselves; an
  expiry with something to do before it (renew a passport, a contract, a
  domain) is an ask with verb `do` ("renew by"), a plain expiry is an
  `expiry` item; money taken automatically is a `charge` item, money the
  recipient must send is an ask with verb `pay`;
  a date with no time is all-day, never a guessed hour; relative words
  resolve against `sent_at` in the given time zone; dates in quoted history,
  signatures or adverts are not items.
- **Veto only, per item** (code may say no, never yes): the quote must appear
  in the message's words (whitespace- and case-normalised); `starts` and
  `due` must parse and lie in `[sent_at − 1 day, sent_at + 731 days]`; an
  `amount`'s digits must appear in the quote or the message's words, else
  the amount (not the ask) is dropped; an item that has
  already ended when it is read is dropped; `kind` must be in the set; an
  `updates`/`cancels` id must belong to the same conversation; a `same_as`
  must be a `task_ref` that was in the list sent. A dropped
  item is logged by reason only.
- **Reasoning budget**: GLM-5.3 Flash shares `max_tokens` with its thinking:
  the call asks for 4,096 (Scout's compile returned unparseable output at
  800).
- **Counted** against the workspace's daily AI limit as background work,
  feature `conversations.read_message`.

### When a layer is missing

- **No `decide` role** (or it failed its answers check): no flags and no
  needs, so L2 never runs (as the summary never runs today). L0 still runs.
- **No `complete` role** (or it may not receive mail): flagged messages show
  "This email has a <kind>" with **Add**, which opens the event form
  prefilled with the kind and the source link; the member types the title
  and time. For `deadline`, Add opens the `tasks` create form (`POST
  /tasks` with the source); until `tasks` exists, deadlines show no Add.

### Cost

Per 1,000 inbound emails with prod's models:

- L0: no model.
- L1: no new call; two more questions in Jev's pass (about $0.04 for the
  whole pass).
- L2: only for **flagged** mail, and only when a need opens or a member
  opens it. It replaces the summary call, which today runs for every need,
  flagged or not, so for most workspaces the read **costs less than today**.
  Unflagged mail and unopened mail without a need never reach the chat
  model. Each call: about 1.5k input tokens, up to 4k output with
  reasoning.
- Caps: the per-workspace daily limit (5,000 calls) and the decide backfill
  window (14 days). `CAMPBOOKS_CONVERSATIONS__DATED__ENABLED` (default on)
  turns L1 flags and the open trigger off.
- **Trade-off**: a booking in mail that raises no need and is never opened
  reaches the calendar only through L0. The big senders of bookings
  (airlines, rail, hotels, ticketing, parcel carriers) mostly send
  schema.org or an `.ics`, so most still land. Recorded in
  `openspec/decisions-needed.md`.

## Data ownership and events

### `schedule` schema (row-level security on `workspace_id`, like every module)

- `calendars`: id, workspace_id, owner member, source (`campbooks` |
  `provider`), connection_id + provider calendar id (provider only), name,
  color, time zone, `syncing`, `writable`, `primary`, sync cursor. Each
  member has exactly one `campbooks` calendar, created on first use.
- `events`: id, calendar_id, kind (`charge` · `appointment` · `meeting` ·
  `travel` · `stay` · `show` · `reservation` · `delivery` · `expiry` · `hold`
  · `none`; deadlines are not events, they are the Due layer from
  `tasks`), title, starts_at/ends_at (UTC) +
  `time_zone` + `all_day` (dates), place, notes, status
  (`confirmed`/`tentative`/`cancelled`), provider id + etag +
  `outbound_pending`, recurrence (RRULE/RDATE/EXDATE text, the provider's
  own), `series_id` + `original_start` for an exception, `source` RecordRef
  (`conversations.message:<id>`, or `tasks.task:<id>` later), `origin`
  (`provider` | `member` | `scout`), `found_by` (`structured` | `model`),
  `pushed_from` (the Campbooks event an "Add to my calendar" copied).
- `member_settings`: time zone (IANA, from the device once, editable),
  working hours (per weekday), kinds Scout adds (default all), per-kind
  "also add to" target calendar (default none).
- `removed_items`: a Scout event the member removed: source message, kind,
  the item's identity. A re-read never adds it again; it is kept as a
  labelled example for the eval set.

Occurrences of a series are expanded at read time, within the asked
window, with an RRULE library (no stored occurrence rows); exceptions
replace their occurrence.

### Published

- `schedule.event_created`, `schedule.event_changed`,
  `schedule.event_cancelled` (v1): event id, calendar id, member, kind,
  starts/ends, all-day, source RecordRef, origin. No title, place or notes
  (PII stays in the module; readers ask the API).
- `schedule.outbound_requested` (v1): an intent for connectors to create,
  update, delete or RSVP an event at the provider (ids, the calendar's
  connection and provider ids, the etag the change was made against). The
  body to write travels as a blob, like `outgoing-message.v1`.

### Consumed

- `connectors.calendar_changed` (v1, new, from connectors): a batch of
  upserted/removed provider events for one calendar, bodies as a blob.
  Handler `schedule.apply_calendar_changes`.
- `connectors.capability_granted` for `calendar`: creates the member's
  provider calendars (primary `syncing` on, others off).
- `connectors.outbound_completed` for calendar intents: stores the new etag
  or the conflict.
- `conversations.message_read` (v1, new): handler
  `schedule.add_read_items` creates, updates or cancels an event on the
  member's Campbooks calendar for each of its `items[]` (idempotent on
  `item_key`; `asks[]` are for `tasks`), unless the member
  removed that item, turned the kind off, or the message is in a shared
  inbox. Shared inboxes add nothing in this change (whose calendar would it
  be?); recorded in `openspec/decisions-needed.md`. The event payload names
  the inbox's personal owner, and the event goes on that member's calendar
  only.

### `conversations` additions

- `conversations.need_summaries` is renamed `conversations.message_reads`
  (migration keeps every row): message id, layer (`structured` / `model`),
  summary (4 locales), `lapses_at`, items, asks, read_at, status
  (`read` / `failed`), attempts, trigger (`need` / `open`).
- `conversations.message_read` (v1), published once per layer that produced
  something: message id, conversation id, inbox id, the inbox's personal
  owner (none for a shared inbox), `layer`, `items[]` (`item_key`, kind,
  starts, ends, all_day, time_zone, updates/cancels), `asks[]` (`ask_key`,
  verb, due, `has_amount`, `same_as`), `summary_ready`. No title, place, quote or
  amount: readers fetch them through the API with their own access.
  `conversations.need_summarised` is kept, published as today, for
  attention.

## Seams with tasks (agreed with the tasks session)

- **One read for both.** `tasks` consumes `message_read.v1` for its
  `asks[]` and never calls `complete` or `decide` on mail. A later
  message's ask names the open task it repeats through `same_as` (the
  `OpenTasks` list sent to the read); with no list, `tasks` falls back to a
  fact rule (same conversation, same verb, open task), so asks need no
  cross-message keys.
- `tasks` owns Do and Pay (its own change). The read keeps the summary and
  `lapses_at` for needs; Do/Pay due dates and amounts come from `asks[]`.
- Due layer: `tasks` publishes one snapshot event, `tasks.task_changed.v1`
  (task, revision, change, state, verb, due_on, due_at, plan, assignee,
  source; no title). Schedule keeps the latest revision per task and draws
  it while the task is open with a due date. `tasks` also exposes a
  `TaskSource` read port for title and visibility. The calendar draws Due markers from
  it. **Follow-up** once those contracts are on `next`.
- Holds: `tasks` → `TimeHeld{hold_id, task_id, start, end, calendar?}`
  (idempotent on `hold_id`) and `TimeReleased{hold_id}`; schedule →
  `event_changed`/`event_cancelled` with the hold's RecordRef (a cancelled
  hold sets the task's plan back to none). Schedule exposes availability as
  a kernel port `free_slots(member, window, duration)`; this change ships
  the port and its query, and the hold events are a **follow-up**.

## Endpoints and scopes

| Endpoint | operationId | Scope |
| --- | --- | --- |
| `GET /api/v1/schedule/events?from=&to=&calendars=` | `schedule.list_events` | `schedule:read` |
| `GET /api/v1/schedule/events/{id}` | `schedule.get_event` | `schedule:read` |
| `POST /api/v1/schedule/events` | `schedule.create_event` | `schedule:write` |
| `PATCH /api/v1/schedule/events/{id}` (one occurrence via `?occurrence=`) | `schedule.update_event` | `schedule:write` |
| `DELETE /api/v1/schedule/events/{id}` (undo token) | `schedule.delete_event` | `schedule:write` |
| `POST /api/v1/schedule/events/{id}/response` | `schedule.respond_event` | `schedule:write` |
| `POST /api/v1/schedule/events/{id}/push` | `schedule.push_event` | `schedule:write` |
| `GET /api/v1/schedule/calendars` | `schedule.list_calendars` | `schedule:read` |
| `PATCH /api/v1/schedule/calendars/{id}` | `schedule.update_calendar` | `schedule:write` |
| `GET /api/v1/schedule/settings` | `schedule.get_settings` | `schedule:read` |
| `PATCH /api/v1/schedule/settings` | `schedule.update_settings` | `schedule:write` |
| `GET /api/v1/schedule/free-slots?from=&to=&minutes=` | `schedule.free_slots` | `schedule:read` |
| `GET /api/v1/conversations/messages/{id}/read` (items and asks with their text) | `conversations.get_message_read` | `conversations:read` |
| `POST /api/v1/conversations/messages/{id}/read` (queue the read on open; no-op when read or not flagged) | `conversations.request_message_read` | `conversations:read` |

A calendar or event the caller may not see answers 404. Every mutating
operation declares its undo in `x-campbooks` (ADR 0013).

## AI: roles, fallbacks, undo

| Use | Role | Not configured | Undo |
| --- | --- | --- | --- |
| `has_dated_thing`, `dated_kind` | `decide` | no flags and no needs, so no L2; L0 still runs | answers are suggestions only |
| The message read (items, asks, summary) | `complete` | flagged mail shows "This email has a <kind> · Add" | each Scout event: Undo for 10 s, then Remove at any time; removal is remembered |
| Event kind of provider events | none | — | the member sets a kind by hand |

Nothing Scout adds goes to a provider calendar unless the member pressed
"Add to my calendar" or set a per-kind target. Removing a pushed event
deletes it at the provider too, with Undo.

## Scout chat (agreed with the cloud #305 session)

- The read ops (`list_events`, `get_event`, `free_slots`, `list_calendars`)
  are `assistant: true`, `mutates: false`. `list_events` filters
  server-side by `kind`, `q` and `limit` over a window of up to 365 days,
  so "when is my flight?" needs no guessed range. `free_slots` works in
  the member's zone and names the calendars it counted.
- Write ops are `risk: low`. Every chat change is a confirmation card in
  v1. `push_event` cards name the target calendar.
- The assistant's chat registry gets three tools: `my_calendar`, `free_time`
  and `read_event`. It also gets an about entry, `schedule.event` →
  `get_event` + `conversations.get_message_read` of its source. Each entry
  is a few lines in `crates/modules/assistant/src/features/chat/`. They
  land in this change if #305 merges first, else in a follow-up.
- `crates/modules/schedule/handbook.md` (≤ 8 KiB, English, concepts, not op
  ids) is composed into Scout's handbook.

## Two-way sync

- **Scopes**: Google `https://www.googleapis.com/auth/calendar.events` and
  `…/calendar.calendarlist.readonly`; Zoho `ZohoCalendar.calendar.READ`,
  `ZohoCalendar.event.ALL`. Added to an existing connection through the
  front door's "extend" path. Sign-in stays mail-only.
- **Reading**: per calendar: Google `events.list` with `syncToken`
  (`singleEvents=false`, so series arrive once with their RRULE and
  exceptions as their own items); a 410 resyncs the calendar. First read:
  events that end from 90 days ago onwards, plus every series still
  recurring. Zoho: its event list by date range with its own change
  marker; the adapter normalises to the same batch. One stream per
  connection and capability, as today; each calendar keeps its cursor in
  the stream's cursor document.
- **Writing**: `schedule.outbound_requested` → connectors' outbound queue
  (the same machinery as mail write-backs: bound to the request, retried,
  never echoed). Updates send `If-Match` with the etag the member saw; a
  412 re-reads the event and the provider's version wins, and the event
  sheet says so ("Changed in Google meanwhile; kept theirs").
- **Round trip**: events Campbooks writes carry
  `extendedProperties.private` `campbooks_event`, `campbooks_kind` and
  `campbooks_source` (Google; Zoho has no equivalent and keeps the link in
  schedule only). A provider event that comes back with these keeps its
  kind and source.

## UI

The Calendar is a new place, so it gets a **visual prototype before code**
(the owner's rule for new visual directions), built on the approved
design-system tokens:

- **Place**: Home · Inbox · **Calendar** in the rail and the phone tab bar;
  `g s` (`nav.schedule` in the shortcut catalogue).
- **Views**: Agenda (default; days with their events, kind icon and colour,
  "from email" mark), Week (time grid, all-day row, now line), Month. `t`
  today, `j/k` next/previous period, `1/2/3` view, `n` new event.
- **Event sheet**: kind chip (editable), title, when (in the member's time
  zone, the event's zone shown when different), place, calendar, "From
  <subject> · <sender>" opening the message in the reader, **Add to my
  calendar** (calendar picker, remembers the last), Edit, RSVP for invites,
  Remove with Undo.
- **Reader strip**: under a message's header, "Scout found: ✈ Flight LIS →
  AMS, Fri 6 Nov 07:40 · Added to your calendar · Undo". With L1 only: "This
  email has a booking · Add".
- **Settings → Calendar**: time zone, working hours, calendars (sync on/off,
  colour), "Scout adds these kinds", "Also add to" per kind, and "Add
  calendar access" for each mail connection without it.
- Kinds have icons and token colours; colours are tokens, not raw values
  (the design-system rule).

## Decisions

- **One read per email, in `conversations`** (the owner's rule, agreed with
  the tasks session). It owns the mail and its words; schedule and tasks
  never see a body. Alternatives: each consumer reads separately (two or
  three `complete` calls per email); schedule reads through a port (a
  second module touching mail content).
- **Jev flags, the read runs only where it already runs or on open.**
  Alternatives: `complete` whenever Jev says yes (rejected by the owner:
  too much mail); candidate dates found by code and picked by Jev (code
  reading dates from prose is what ADR 0018 forbids); Jev only, with the
  member typing dates (kept as the fallback).
- **Obligations are asks, never items.** One obligation cannot arrive
  twice in one read; `tasks` owns it, and the calendar shows it as Due.
- **Scout's events go to the member's own Campbooks calendar, not the
  provider.** The owner's choice; it keeps a wrong read from landing in
  Google, where other people see it.
- **Series stored once, expanded on read.** Alternatives: `singleEvents=true`
  with a window (Rails' way: rows for every occurrence and a moving window
  to maintain, and an infinite series has no end).
- **Deadlines belong to `tasks`.** Agreed with the tasks session; the
  calendar shows them as a projection.

## Risks / Trade-offs

- **Jev's gate is unmeasured.** Too low: L2 cost; too high: missed events.
  Contained by the env threshold, by recording every answer, and by an eval
  set whose labels come from the owner or from members' Undo/Remove, never
  from the agent that writes the code (the lesson of #447).
- **The model invents or misreads a date.** Contained by the verbatim quote
  check, the date window, the reader showing the quote ("Scout read: 'Your
  flight departs on 6 November at 07:40'"), and by never writing to a
  provider without the member.
- **Forged markup.** L0 only for aligned DMARC passes and never for mail
  Jev calls suspicious.
- **Scope creep in one PR.** The owner chose one change; the tasks list is
  ordered so each slice is reviewable on its own commit range.
- **Zoho Calendar is unverified** against a live grant (as it was in
  Rails). The adapter ships behind `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR`
  (default off) until a hand test passes.
