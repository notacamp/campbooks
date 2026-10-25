## ADDED Requirements

### Requirement: Holding time for a task
`tasks.hold` SHALL take the revision the caller saw, a start and an end (15 to
240 minutes, starting in the future) and optionally one of the caller's
writable calendars, and SHALL, in one transaction, make an event of kind
`hold` on that calendar (the member's Campbooks calendar when none is given)
through the kernel `Holds` port, titled with the task's title and linked to
the task, and plan the task for the local day of the start. A task SHALL have
at most one hold: holding again SHALL release the previous one. Only an open
`do` task assigned to the caller SHALL be held (`pay_not_held` for a bill).
`tasks.release_hold` SHALL remove the event and set the plan to none. Each
SHALL declare the other as its undo, append one history entry (`held`,
`released`) and publish `tasks.task_changed` with the change `planned`.
`TaskView` SHALL carry the hold (event, start, end) while there is one.

#### Scenario: Holding Wednesday 10:00
- **WHEN** Ana holds 10:00–10:30 on Wednesday for "Send the signed lease"
- **THEN** her calendar has a held block "Send the signed lease" 10:00–10:30, the task is planned for Wednesday with that hold, and its history holds `held`

#### Scenario: Holding again moves it
- **WHEN** Ana holds Thursday 14:00–15:00 for the same task
- **THEN** the Wednesday block is gone and the task is held on Thursday

#### Scenario: Undo
- **WHEN** Ana holds a slot and presses Undo
- **THEN** the block is removed, the task has no plan, and its history holds `held` then `released`

#### Scenario: A bill
- **WHEN** a client asks to hold time for a pay task
- **THEN** the answer is 400 `pay_not_held` and nothing changes

### Requirement: A hold follows its calendar block
The tasks module SHALL follow `schedule.event_changed`,
`schedule.event_cancelled` and `schedule.event_created` for events whose source
is a task: a removed or cancelled block SHALL release the hold and set the plan
to none; a moved block SHALL move the hold and the planned day; a restored
block SHALL become the hold again when the task is open, its owner's, and not
held. Each SHALL be recorded with the actor `calendar` and be idempotent.

#### Scenario: Moved in Google
- **WHEN** Ana drags the held block from Wednesday 10:00 to Thursday 09:00 in Google and the calendar syncs
- **THEN** the task is held Thursday 09:00–09:30 and planned for Thursday, and its history says the calendar changed it

#### Scenario: Removed on the calendar
- **WHEN** Ana removes the held block in Campbooks' calendar
- **THEN** the task has no hold and no plan

### Requirement: Closing a task frees its time
Marking a task done or not needed, or handing it to a teammate, SHALL release
its hold in the same transaction when the hold has not started yet, and SHALL
keep a hold that has started or passed.

#### Scenario: Done early
- **WHEN** Ana marks "Send the signed lease" done on Tuesday while it is held Wednesday 10:00
- **THEN** the Wednesday block is removed from her calendar
