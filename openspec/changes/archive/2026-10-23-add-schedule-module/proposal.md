# Calendar: the schedule module and dated things read from email

Modules: new `schedule` (crate `campbooks-schedule`, schema `schedule`,
client module `schedule` with the Calendar place), `conversations` (the one message read: dated items, asks and the need
summary in a single `complete` call, published as
`conversations.message_read`; it replaces `summarize_need`), `campbooks-ai` (two triage questions in
the existing decide pass). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/42. Entry in
`docs/domain-map.md` → *schedule*. New ADR 0021 (machine-readable dates
may assert). Seams with `tasks` (campbooks-cloud#43, built in parallel) are
agreed and listed in design.md; the parts that need `tasks` contracts land
with that module, not here. **Shipped in two changes:** this one is the server
side, the reading of dates from email and the Calendar screens; Google and
Zoho calendar sync, and everything that needs a provider calendar, is
`add-calendar-sync`.

## Why

Email is where most of a person's dates arrive: a flight confirmation, a
dentist's booking, a concert ticket, "your plan renews on 3 November", "pay
by the 30th", a parcel arriving Thursday. Today Next reads every message
and does nothing with any of those dates. There is no calendar either: the
member's calendar is nowhere in Next, so Scout cannot say what the
day looks like or what is coming.

The Rails app had a calendar, and its event-from-email was senseless: the
title was the subject, the time was "the day after the email at 09:00"
unless a regex found "3pm", and a regex for times decided whether to offer
an event at all. ADR 0018 rules that out for Next: a model reads content;
code only reads structured facts and only says no.

The owner's requirements for this change:

- Events have types: deadlines, charge days, shows, and the other kinds of
  dated thing mail carries.
- An event read from an email keeps a link to that email.
- **Cost-conscious: Jev (the `decide` model) is the first layer; a
  conversational model runs only when Jev cannot do the job.** Refined with
  the owner through the tasks session: each email gets one Jev pass and at
  most one chat read, covering everything in it, and most mail never
  reaches the chat model.
- Two changes: this one ships the Campbooks calendar, the extraction and the screens; two-way provider sync follows as `add-calendar-sync`.
- An event Scout finds lands on the Campbooks calendar at once, marked "from
  email", with Undo; nothing is written to Google or Zoho until the member
  says so ("Add to my calendar", or a per-kind rule, both in `add-calendar-sync`).

## What Changes

- **New module `schedule`**: calendars (provider calendars plus each
  member's own Campbooks calendar), events with a **kind**, recurring series
  expanded into occurrences, member time zone and working hours,
  availability. Scopes `schedule:read`, `schedule:write`. Publishes
  `schedule.event_created`, `schedule.event_changed`,
  `schedule.event_cancelled` (v1, ids only).
- **Event kinds** (a fixed, typed set): `charge`, `appointment`, `meeting`, `travel`, `stay`, `show`, `reservation`,
  `delivery`, `expiry`, `hold`, and `none` for provider events nobody typed.
  Obligations (pay by, submit by, renew by) are not events: they are
  *asks*, owned by `tasks`, and the calendar shows them as a Due layer
  projected from `tasks`.
- **The dated-things read, layered by cost** (design.md → *Three layers*):
  1. **L0, free**: an iCalendar part (`text/calendar`) or schema.org
     reservation markup in a message from an authenticated sender becomes
     typed items with no model at all (ADR 0021).
  2. **L1, Jev, no new call**: two questions join the existing decide pass:
     `has_dated_thing` (noul) and `dated_kind` (choice). Jev's answers gate
     everything after them.
  3. **L2, the message read, at most one `complete` call per email**: it
     replaces `summarize_need` and returns everything at once: items
     (things at a time), asks (obligations, for `tasks`) and the need
     summary. It runs only when Jev's pass flagged the email (asks
     something, a deadline, a dated thing) **and** a need opened or the
     member opened the email. Unflagged mail and unopened mail without a
     need never reach the chat model. Code checks each item and ask (quote
     verbatim in the message, date parses and lies in a sane window, kind in
     the set, amount digits present) and only drops.
  When `complete` is not configured, Jev's answer alone still shows "This
  email has a <kind>" in the reader with Add, which opens a prefilled form.
- **`conversations.message_read` (v1)**: one event per read, ids and enums
  only. `schedule` turns its items into events on the member's Campbooks
  calendar; `tasks` (its own change) turns its asks into tasks. Neither
  calls a model on mail.
- **Two-way sync is `add-calendar-sync`**, not this change. Here the
  `schedule` module already holds the code for it (applying provider
  batches, `outbound_requested`, `push_event`) but nothing publishes the
  provider side yet, so members have only the Campbooks calendar.
- **Calendar place** in the app (Home · Inbox · **Calendar**): agenda, week
  and month views, an event sheet with its kind, time, place and "From
  <email>" link, the reader's dated-things strip, settings for time zone,
  working hours, calendars and kinds. Prototype first (design.md →
  *UI*).

## Capabilities

### New Capabilities

- `schedule`: calendars, typed events, recurrence, time zone and working
  hours, availability, events from dated things, provider write-back
  intents.
- `calendar-ui`: the Calendar place, the event sheet, the reader's
  dated-things strip and the calendar settings.

### Modified Capabilities

- `conversations`: two questions in the decide pass; L0; the message read
  replacing the need summary (narrower trigger, items and asks); publishing
  `message_read`; the `OpenTasks` port it reads.
- `ai-decide-facts`: "No content rules" names machine-readable calendar
  data as structured data, not content (ADR 0021).

## Impact

- Adds a module (`docs/domain-map.md` → *schedule*). No module imports
  another: `schedule` consumes `conversations.message_read` (and, with
  `add-calendar-sync`, `connectors.calendar_changed`), and asks connectors
  for write-backs
  through an outbound-requests event, the way `conversations` does.
- **Cost**: L1 adds two questions to a call that already happens (Jev:
  about $0.04 per 1,000 emails for the whole pass). L2 replaces the summary
  call and runs on fewer emails than it does today. design.md → *Cost*.


## Non-goals

- Google and Zoho calendar sync (`add-calendar-sync`), Microsoft 365 / Graph and CalDAV calendars (no Microsoft mail adapter
  exists yet; CalDAV follows IMAP).
- Push notifications from providers (Google `events.watch`): adaptive
  polling covers it, as for mail today.
- Editing a whole recurring series from Campbooks ("all events"). This
  change edits single events and single occurrences; series edits stay in
  the provider.
- Sharing calendars between members (reader/writer/manager): calendars are
  their grantor's, as inboxes are.
- Typing provider events with Jev (they keep `none` unless Campbooks wrote
  them).
- Tasks, holds and the Due layer's data: owned by `tasks`
  (campbooks-cloud#43). This change publishes and consumes nothing of
  `tasks` yet; follow-up issues wire `TimeHeld`/`TimeReleased`, the
  `TaskSource` port and the Due layer once the `tasks` contracts are on
  `next`.
- A "coming up" section on Home.
