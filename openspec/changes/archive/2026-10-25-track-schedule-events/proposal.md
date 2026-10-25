# Track the schedule module's changes

Module: `schedule` (`crates/modules/schedule/`: its operations, its jobs
and its event handlers), with the tracking plan (`tracking/`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/349, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. The schedule module
holds each member's calendars and events: what they make, move, rename,
delete and answer, what Scout reads out of their mail, and which calendars
they turn on. It landed after the tracking plan, so none of it is tracked
and the record cannot answer "who moved this meeting, and from when", "how
often does Scout get an event's kind wrong" or "how many members turn on a
second calendar". Full coverage becomes a CI failure with
campbooks-cloud#332, so every operation, job and handler of the module
needs a Coverage row first.

The connectors module already tracks the provider's side of a calendar:
each calendar sync run (`connectors.stream_synced`) and how each change the
schedule module asked for ended at the provider
(`connectors.calendar_change_completed`). The schedule events are the
other half: the member's intent, and what the schedule module's own
records became.

## What Changes

- Tracking plan version 9 (`tracking/patches/0009-schedule.yaml`): 16 new
  live events of `schedule`:
  - events of a calendar: `calendar_event_created`, `calendar_event_edited`
    (which fields, by name), `calendar_event_moved` (with the times
    before), `calendar_event_cancelled`, `calendar_event_deleted`,
    `calendar_event_restored`, `calendar_event_responded` (with the answer
    before), `calendar_event_pushed` and `calendar_event_unpushed` ("Add to
    my calendar" and its undo);
  - calendars: `calendar_added` (a calendar grant), `calendar_enabled`,
    `calendar_disabled`, `calendar_recoloured`, `calendar_removed` (with
    its connection);
  - `settings_updated` (which settings, by name);
  - `provider_changes_applied`: one summary per batch of the provider's
    changes the module applied, with counts.
- The records `schedule.calendar_event`, `schedule.calendar_event_times`,
  `schedule.calendar` and `schedule.settings`, their enums, and a Coverage
  row for each of the module's 9 mutating operations, 3 jobs and 7 event
  handlers.
- Each change is tracked with `Tracker::track` in the transaction that
  makes it, with a snapshot read in that transaction. A change that changes
  nothing tracks nothing.
- The module gets a test kit (`testkit.rs`) and tests that read every event
  back from `kernel.tracked_events`.

No new endpoint, scope, migration, dependency or domain event. No change to
what any endpoint returns.

## Non-goals

- The other modules (#325 to #331) and the hard coverage gate (#332).
- One event per event of a provider's batch: a backfill applies thousands;
  the batch's summary carries the counts.
- What happens at the provider: `connectors.calendar_change_completed`
  already records how each change ended there, and
  `connectors.stream_synced` each calendar sync run.
- Reads (`list_events`, `get_event`, `free_slots`, `list_calendars`,
  `get_settings`) and the ports (`time_zones`, `availability`,
  `calendar_requests`): they change nothing.
- The member's Campbooks calendar being made the first time something needs
  it: a side effect of whatever needed it, often a read.
- Any personal data or content: no title, description, place, notes,
  quote, attendee, meeting link, calendar name, colour value, time zone
  name, provider id, etag or iCalendar UID.
