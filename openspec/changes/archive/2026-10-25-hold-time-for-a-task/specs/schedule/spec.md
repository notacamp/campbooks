## ADDED Requirements

### Requirement: Holds for tasks
The schedule module SHALL implement the kernel `Holds` port: `hold` SHALL make
an event of kind `hold`, busy, made by the member, linked to the task, on one
of the member's writable calendars (their Campbooks calendar when none is
named), asking a provider calendar for it like any event the member makes, and
publish `schedule.event_created`; `release` SHALL remove it as `delete_event`
does and publish `schedule.event_cancelled`, and SHALL do nothing for an event
that is gone or is not that member's hold. A calendar that is not the member's
SHALL be refused as not found, and one that cannot be written as
`calendar_read_only`.

#### Scenario: On Google
- **WHEN** the tasks module holds 10:00–10:30 on Ana's Google work calendar
- **THEN** the hold event is stored, marked pending, and asked of Google in the same transaction

#### Scenario: Someone else's calendar
- **WHEN** a hold names a calendar of another member
- **THEN** nothing is made and the call answers not found
