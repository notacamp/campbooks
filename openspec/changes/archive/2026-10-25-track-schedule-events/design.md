# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the schedule module
(`crates/modules/schedule/`).

## Data ownership and events

The module owns the schema `schedule`. Every snapshot is read from its own
rows (`schedule.events` joined to `schedule.calendars`,
`schedule.calendars`, `schedule.member_settings`) in the transaction of the
change, by `domain/tracked.rs`. Every event belongs to the workspace of the
calendar. Events go to `kernel.tracked_events`; nothing else is written. No
domain event, endpoint, scope or AI role changes; new queries are runtime
`sqlx::query`/`query_as` (no `.sqlx` file changes).

What an event may hold: the module's ids (event, calendar, series, the
Campbooks event a copy copies), the member's user id (pseudonymous), the
connection's id, the id of the email an event was read from (a
`conversations.message` id), enums (kind, origin, how Scout found it,
status, the member's answer, a pending change, a notice, where a calendar
lives, its provider), booleans (all-day, recurring, busy, at the provider,
on, writable, primary), counts and times (start, end, created, deleted).
Never a title, description, place, notes, the quote Scout read, an
attendee, a meeting link, a calendar's name or colour value, a time zone
name, a provider's calendar or event id, an etag or an iCalendar UID.

- **Time zones** are left out, even as an IANA `code`: an event's zone comes
  from the provider or the sender as given, and a member's zone is where
  they live. Both say where a person is; no question the record answers
  needs them. `settings_updated` names the field (`time_zone`, or
  `device_time_zone` when it was taken from the device), never the value.
- **Colours**: a calendar's colour is a provider's or the member's choice
  of `#rrggbb`. Harmless, but nothing needs the value: `calendar_recoloured`
  says that it changed.
- **Attendees**: the module stores no attendee list, so there is no count to
  carry; the member's own answer (`response`) is the invitation's only
  trace.
- **Source**: an event read from an email carries the message's id
  (`source_message_id`) when its source is a `conversations.message` with a
  UUID id; any other source line (one a provider's private properties
  carry) is left out.

## Events

All workspace events. "Event" below is a calendar event; property names
never say `event` or `event_id` (the warehouse flattens them beside the
envelope).

| Event | Fires when | Actor | Snapshot and properties |
| --- | --- | --- | --- |
| `schedule.calendar_event_created` | `schedule.create_event` makes an event, or `schedule.add_read_items` adds one Scout read in an email to the inbox owner's Campbooks calendar | `user`, `assistant`, `automation`, `system` | `calendar_event` |
| `schedule.calendar_event_edited` | `schedule.update_event` changes a field other than the times (of an event, or of one occurrence of a series: its exception); `schedule.add_read_items` changes an event a later email updates, or links an invitation already on a calendar to its email (`source`); nothing changed tracks nothing | `user`, `assistant`, `automation`, `system` | `calendar_event` (after), `fields` (list of `schedule.event_field`), `before_kind`? (when the kind changed) |
| `schedule.calendar_event_moved` | the same seams change its start or end (or make it all-day) | `user`, `assistant`, `automation`, `system` | `calendar_event` (after), `before` (`schedule.calendar_event_times`) |
| `schedule.calendar_event_cancelled` | `schedule.add_read_items` cancels an event a later email cancels | `system` | `calendar_event` (after) |
| `schedule.calendar_event_deleted` | `schedule.delete_event` deletes it (a series whole); the delete can still be undone | `user`, `assistant`, `automation` | `calendar_event` (after, `deleted_at` set) |
| `schedule.calendar_event_restored` | `schedule.restore_event` brings a deleted event back | `user`, `assistant`, `automation` | `calendar_event` (after), `remade_at_provider` (the delete had reached the provider, so it is made there again) |
| `schedule.calendar_event_responded` | `schedule.respond_event` changes the member's answer; the same answer again tracks nothing | `user`, `assistant`, `automation` | `calendar_event` (after), `before` (`schedule.response`) |
| `schedule.calendar_event_pushed` | `schedule.push_event` ("Add to my calendar"), or `schedule.add_read_items` for a kind the member set an "also add to" calendar for, copies a Campbooks event to a provider calendar | `user`, `assistant`, `automation`, `system` | `calendar_event` (the copy; `pushed_from_id` the Campbooks event) |
| `schedule.calendar_event_unpushed` | `schedule.unpush_event` takes the copy away; the Campbooks event is back | `user`, `assistant`, `automation` | `calendar_event` (the Campbooks event), `copy_id`, `copy_calendar_id` |
| `schedule.calendar_added` | `schedule.grant_calendars` makes a calendar of a calendar grant (one per calendar the module did not have) | `system` | `calendar` |
| `schedule.calendar_enabled` | `schedule.update_calendar` turns a calendar on | `user` | `calendar` |
| `schedule.calendar_disabled` | `schedule.update_calendar` turns a calendar off | `user` | `calendar` |
| `schedule.calendar_recoloured` | `schedule.update_calendar` changes its colour | `user` | `calendar` |
| `schedule.calendar_removed` | `schedule.forget_connection` deletes a removed connection's calendars, one per calendar | `system` | `calendar` (as it stood), `calendar_events` (events deleted with it) |
| `schedule.settings_updated` | `schedule.update_settings` changes at least one setting | `user` | `settings` (after), `fields` (list of `schedule.settings_field`) |
| `schedule.provider_changes_applied` | `schedule.apply_calendar_changes` applied a batch that created, changed or cancelled at least one event; a batch for a calendar that is off, or one that changed nothing, tracks nothing | `system` | `calendar`, `created`, `changed`, `cancelled`, `full` |

A client token acts for no member and has no calendar (every operation
answers 403), so `client` is no actor type of these events.

Records:

- `schedule.calendar_event`: `id`, `calendar_id`, `member_id`
  (pseudonymous), `calendar_source`, `provider`?, `kind`, `origin`,
  `found_by`?, `status`, `all_day`, `starts_at`?, `ends_at`? (timed),
  `starts_on`?, `ends_on`? (all-day), `recurring`, `series_id`?, `busy`,
  `response`?, `pending_change`?, `notice`?, `at_provider`,
  `source_message_id`?, `pushed_from_id`?, `pushed_to_id`?, `deleted_at`?,
  `created_at`, `updated_at`.
- `schedule.calendar_event_times`: `all_day`, `starts_at`?, `ends_at`?,
  `starts_on`?, `ends_on`?.
- `schedule.calendar`: `id`, `member_id` (pseudonymous), `source`,
  `provider`?, `connection_id`?, `syncing`, `writable`, `primary`,
  `created_at`.
- `schedule.settings`: `member_id` (pseudonymous), `time_zone_set`,
  `working_hours_set`, `working_days`, `scout_kinds` (list),
  `also_add_to_kinds` (list).

Enums: `schedule.event_kind` (charge, appointment, meeting, travel, stay,
show, reservation, delivery, expiry, hold, none), `schedule.event_origin`
(provider, member, scout), `schedule.found_by` (structured, model),
`schedule.event_status` (confirmed, tentative, cancelled),
`schedule.response` (needs_action, accepted, tentative, declined),
`schedule.event_notice` (kept_theirs, not_saved), `schedule.calendar_source`
(campbooks, provider), `schedule.provider` (google, zoho),
`schedule.calendar_change` (create, update, update_occurrence, delete,
respond), `schedule.event_field` (title, kind, place, notes, time_zone,
busy, status, source), `schedule.settings_field` (time_zone,
device_time_zone, working_hours, scout_kinds, also_add_to).

### Edited and moved

A change of times is the transition that matters (a meeting moved, a
flight delayed), so it is its own event with the times before. Any other
change names its fields, never their values; the kind is the exception,
with `before_kind`, because a corrected kind is how often Scout got it
wrong. One `PATCH` that renames and moves an event tracks both.

### The provider's changes: a summary per batch

A calendar sync applies the provider's batches; a backfill is thousands of
events, so `apply_calendar_changes` tracks one `provider_changes_applied`
per batch with counts, never one event per event. Connectors'
`connectors.stream_synced` counts what a run read at the provider; it
cannot know what the schedule module made of it (an event new to the
member, one changed, one cancelled, or nothing: the same version again, a
calendar that is off, a member's waiting change kept), so the summary is
the schedule side of the same run.

### How the member's changes end at the provider

`apply_outcome` stores how a change ended (applied, superseded, skipped,
failed): it clears the pending mark, keeps the provider's version, or
undoes what the provider did not take. That ending is one
`connectors.calendar_change_completed` per change already, with the
schedule's request, calendar and event ids, the change, the outcome and
the reason, so its Coverage row names that event instead of tracking a
copy. `release_delete`, which asks for a delete once its undo window has
passed, likewise: the member's delete is `calendar_event_deleted`, how it
ends at the provider `connectors.calendar_change_completed`.

### Where each is tracked

- **Snapshots.** `domain::tracked` reads `calendar_event` (an event joined
  to its calendar), `calendar` and the settings from the module's rows, and
  maps the module's strings to the plan's enums.
- **Operations.** Each handler tracks after its last write, before
  `commit`: `create_event`, `delete_event`, `restore_event`,
  `respond_event` (when the answer changed), `unpush_event`,
  `update_calendar` (`enabled`/`disabled` when `syncing` changed,
  `recoloured` when the colour did), `update_settings` (when a setting
  changed). `update_event` compares the event before and after
  (`tracked::changes`). `push_row` (shared by `push_event` and Scout's
  "also add to") tracks `calendar_event_pushed`.
- **Handlers.** `add_read_items` tracks what each item did: created,
  cancelled, or changed (`tracked::changes`, the same comparison).
  `grant_calendars` tracks each calendar its insert made.
  `apply_calendar_changes` counts what it created, changed and cancelled
  (now including the provider's removals and a full read's leftovers) and
  tracks the summary at the end. `forget_connection` reads the
  connection's calendars and their event counts before it deletes them.

## Coverage

Operations (`openapi/openapi.json`, tag `schedule`; the reads need no row):

| Operation | Events |
| --- | --- |
| `schedule.create_event` | `calendar_event_created` |
| `schedule.update_event` | `calendar_event_edited`, `calendar_event_moved` |
| `schedule.delete_event` | `calendar_event_deleted` |
| `schedule.restore_event` | `calendar_event_restored` |
| `schedule.respond_event` | `calendar_event_responded` |
| `schedule.push_event` | `calendar_event_pushed` |
| `schedule.unpush_event` | `calendar_event_unpushed` |
| `schedule.update_calendar` | `calendar_enabled`, `calendar_disabled`, `calendar_recoloured` |
| `schedule.update_settings` | `settings_updated` |

Event handlers:

| Handler | Events / exemption |
| --- | --- |
| `schedule.grant_calendars` | `calendar_added` |
| `schedule.apply_calendar_changes` | `provider_changes_applied` |
| `schedule.apply_outcome` | `connectors.calendar_change_completed` (tracked by connectors in the transaction that publishes the outcome this handler applies) |
| `schedule.add_read_items` | `calendar_event_created`, `calendar_event_edited`, `calendar_event_moved`, `calendar_event_cancelled`, `calendar_event_pushed` |
| `schedule.forget_connection` | `calendar_removed` |
| `schedule.forget_member` | exempt: consequence of workspaces.member_left, tracked there: the departed member's calendars, events and settings go |
| `schedule.schedule_workspace_purge` | exempt: projection of workspaces.workspace_deleted: it only schedules the purge |

Jobs:

| Job | Events / exemption |
| --- | --- |
| `schedule.release_delete` | `connectors.calendar_change_completed` (the member's delete is `calendar_event_deleted`; this asks for it at the provider once the undo window passed, and connectors tracks how it ends) |
| `schedule.purge_workspace` | exempt: maintenance: a deleted workspace's calendars and events deleted; the deletion is tracked by workspaces |
| `schedule.retag_bindings` | exempt: maintenance, no action of anyone: MACs of waiting change requests re-tagged after a key rotation |

The module has no schedule (cron).
