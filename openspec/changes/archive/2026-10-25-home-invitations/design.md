# Design

## Context

The schedule module already keeps the member's answer to an invitation
(`schedule.events.response`, from the provider's batch) and answers it
(`schedule.respond_event`, written back through connectors). Home's list is
`attention.list`, a projection of other modules' events (needs, tasks).
Nothing told attention that an invitation awaits an answer.

## Data ownership

- **schedule** owns invitations. Migration
  `20261108010000_schedule__invitations.sql` adds to `schedule.events`: the
  organiser as the provider gives it (`organizer_name`, `organizer_address`,
  `organizer_self`, and `organizer_person`, whether a person rather than a
  machine organises it), `invitation_revision` and `invitation_open`; and the
  table `schedule.invitation_backfills` (workspace ids) for the backfill. The
  batch's `organizer` (already in `calendar-change-batch` v1) is now read.
  `EventView` gains `organizer` (name, address, person), never shown for an
  event the member organised.
- **attention** owns the Decide items: no migration. An item is about
  `schedule.event:<id>` (its cause too), in the container
  `schedule.member:<member>` (only that member sees inside: the schedule
  module's `Visibility` port, registered by the server like the tasks
  module's), reason `decide`, trust `sure`, `due_on` the day the event
  starts, `lapses_at` when it stops awaiting an answer, `from_person` who
  organises it.

## What an invitation is

`domain::invitations::standing` (schedule), first answer wins:

| Not an invitation when | Closed because |
|---|---|
| the member's response is not `needs_action`, or the member organises it | `answered` |
| it is cancelled | `cancelled` |
| it is deleted, or pushed away | `removed` |
| it is on the Campbooks calendar, its calendar does not sync, or it is one occurrence of a series | `hidden` |
| it started (an all-day event: its first day, in the member's zone, is over) | `started` |

Judgment calls: an invitation that has started simply clears (it is never
Now or overdue); an exception of a series is not one of its own, its answer
is the series'; a series is one invitation, dated by its first occurrence.

## The contract: a new event

`schedule.invitation_changed` v1
(`contracts/schedule.invitation_changed.v1.schema.json`): `workspace`,
`event`, `calendar`, `owner`, `revision`, and either `invitation` (`on`,
`lapses_at`, `from_person`) or `closed_because`. A new event rather than
optional fields on `schedule.event_created|changed|cancelled`:

- those three carry no revision, so a consumer cannot be order-proof with
  them, and they are not published when only the response changes (a batch
  publishes `event_changed` only when the provider's fields or the kind
  changed);
- an invitation's facts (the day in the member's zone, the lapse time, who
  organises it, whether it is open) are the schedule module's own reading,
  worked out once where the zone and the organiser live, not by each
  consumer;
- one event with one meaning keeps the three event contracts unchanged.

No personal data: ids, a day, a time, flags and an enum
(`xtask` payload audit: nothing to allow).

## Publishing and order-proofing

`invitations::announce(event)` runs in the transaction of every change that
can make an event an invitation or stop it being one: the publish helpers
(`publish::created|changed|cancelled`, so creating, applying a batch,
deleting, restoring, the outcome of a provider change), a batch that only
changes the response or the organiser, `respond_event`, a calendar turned on
or off (`announce_calendar`), and a removed connection or a member who left
(`close_all`, before the calendars go). It publishes when the event awaits an
answer now or did before (`invitation_open`), with
`invitation_revision + 1`, under the event's row lock. A cheap check
(`concerns`: open before, or `needs_action`) leaves every other event alone.

`attention.project_invitation` applies an event only when its revision is
newer than the item's; a close that arrives before its open leaves a
tombstone at its revision, and is not announced to anyone (no item was
seen). An item reopened (an answer undone) keeps its id and `since`.

## Bands

`BandInput.event`: an invitation's date is the day its event happens. Rules
3, 7 and 10 name it so (`event_today`, `event_tomorrow`, `event_on` with the
day), keeping the rule order; it is never overdue (a day already begun in
another zone is today), and rule 8 (a person waiting for a choice) does not
apply, so an invitation 12 days ahead from a person is Whenever, not This
week. Inside a band the order is unchanged (dated first, soonest first,
people first, oldest first).

## Answers and Undo

Accept, Decline and Maybe call `schedule.respond_event`, the same operation,
permission checks and outbound request as the Calendar; no second write path.
Its Undo is the operation's own: `previous_response` sent back.

Until now an answer was asked of the provider at once, and an answer back to
"not answered" was never told to it (no provider takes an answer back;
Microsoft Graph has no way to). So Undo either met `409 change_pending` or
left the provider holding the answer, which the next sync brought back. Now,
as a delete does, an answer on a provider calendar is stored and marked
pending, and `schedule.release_response` (10 seconds later, unique per
event) asks the provider for the answer the event holds then. Within the
window a new answer replaces the one waiting and "not answered" withdraws it;
once the request is asked of connectors another answer waits for it to
finish (`409`, as before). Home's toast stays as long as the window, so its
Undo never reaches the organiser. An answer the provider already has is
still only stored here when taken back (unchanged; a non-goal).

## The backfill

The migration queues every workspace with an unanswered event. The minute
dispatcher `schedule.dispatch_invitation_backfills` (maintenance scope: reads
the table's ids only; `architecture.toml` entry) enqueues
`schedule.announce_invitations` in each, which announces every event that
could still be one (not started) from the module's own rows and removes the
workspace's row: no resync, no cross-schema SQL, no model. Organisers are
learnt with each event's next change from the provider; until then
`from_person` is unknown (Whenever-friendly, never "waiting") and Home names
the calendar instead of the organiser.

## Client

- **schedule** exports `useInvitations(workspaceId, refs, { now })`: the
  events of the invitation items (one `schedule.list_events` window from
  today to the last invitation's day, at least the 8 days `useHomeDay`
  reads, so both share one query), each as a row in words: the organiser
  (name, else address, else the calendar's name), "Invitation: <title> ·
  <place>" (a web place by its host), its time ("14:00 – 15:00") in the
  member's zone and language, at the row's end (the day is in the eyebrow;
  every line of it wraps or is short, so the copy fits in every language),
  whether its calendar can be written, Open in Calendar, and the free/busy
  fact ("Free then", or "Busy then" and the other event's title). And `useInvitationAnswers`, the answers with their toast ("Accepted:
  <title>", Undo for the window's 10 seconds).
- **Free or busy** is worked out on the client from the same events: another
  event that overlaps it, busy, timed, not declined (held time counts: the
  member set it aside; all-day events and those marked not busy do not, as
  free time counts them). The data is already there, so no endpoint.
- **No event twice.** While an invitation is a Decide row, `useHomeDay`
  leaves its event out of Next up, today's rows and Coming up (`hide`); once
  answered, it is a calendar row again.
- **attention** places the rows: a third kind of entry beside needs and
  tasks, keys 1/2/3 for the answers, S for Snooze, Enter opens Calendar on
  its day; a calendar that cannot be written offers no answers (Snooze,
  the member's own, stays, as on a read-only inbox).

## Scopes

Unchanged: `attention.list` (`attention:read`), `schedule.list_events`
(`schedule:read`), `schedule.respond_event` (`schedule:write`),
`attention.snooze` (`attention:write`).

## AI

None. Every fact comes from the provider's own fields (the response, the
organiser's address, the times); the machine-sender test is the facts
strategy's address check (ADR 0018), and nothing reads an event's words.
Nothing is changed without the member: an answer is their press, with Undo.

## Tracking coverage

Checked `tracking/tracking-plan.xlsx` (Coverage) and `docs/tracking.md`. No
tracked event is added: an answer is `schedule.respond_event`, which tracks
`schedule.calendar_event_responded` (an Undo tracks the answer before).
Patch `tracking/patches/0014-home-invitations.yaml` (plan version 14) adds
Coverage rows:

| Kind | Name | Events / exemption |
|---|---|---|
| job | `schedule.release_response` | `connectors.calendar_change_completed` (as `release_delete`) |
| schedule | `schedule.dispatch_invitation_backfills` | maintenance: only enqueues the jobs that do the work |
| job | `schedule.announce_invitations` | maintenance: invitations stored before announced once |
| handler | `attention.project_invitation` | projection of `schedule.invitation_changed` |
