## MODIFIED Requirements

### Requirement: Items are references
An attention item SHALL hold the record it is about, the container that
decides who may see it, the record that caused it, a reason (`review`,
`reply`, `decide`, `do`, `pay`, `worth_a_look`, `waiting` or `nudge`), a trust (`sure` or `check`), an optional due
date, an optional lapse time (when what it asks stops being possible),
whether it is urgent, and when its cause arrived. It SHALL hold no
names, addresses, subjects or message text. Every table of the `attention`
schema SHALL be isolated per workspace by forced row-level security.

#### Scenario: A reply need from Marta
- **WHEN** an item is projected for a conversation in which Marta Ferreira asks for a reply
- **THEN** the stored item and its events contain the conversation, inbox and message ids, the reason `reply` and the due date, and neither Marta's name, her address nor her sentence

#### Scenario: A need with a lapse time
- **WHEN** `need_changed` carries a `do` need lapsing at 09:10 UTC
- **THEN** the item holds that lapse time; an event without one leaves the item with none

#### Scenario: Waiting on Ana
- **WHEN** `need_changed` carries a `waiting` need due on Thursday
- **THEN** the item holds the reason `waiting` and Thursday, and neither Ana's name nor Ana's address

### Requirement: Listing what needs the caller
`GET /api/v1/w/{workspace_id}/attention` (`attention.list`, `attention:read`)
SHALL return the caller's open items, leaving out items in containers the
caller cannot see (the kernel's `Visibility` port), items the caller snoozed
until a later time, items whose lapse time has passed, `waiting` items, and items other than
`pay` and `nudge` more than 7 days past their due date (a bill or a nudge
stays until a member answers it). Items
SHALL be ordered by reason (`review`, `reply`, `decide`, `do`, `pay`, `nudge`, `worth_a_look`), then
overdue first, then by due date, then urgent first, then oldest cause first,
and the response SHALL carry the count per reason. The caller's day (for what
is overdue and for Done today) SHALL follow `utc_offset`, the caller's offset
from UTC in minutes (default 0; beyond 14 hours, 400 `invalid_offset`). With
`state=cleared` it SHALL return the items cleared since the start of the
caller's day, newest first, with what cleared them and who, leaving out those
cleared as `superseded`, `rule` or `lapsed`. With `state=waiting` it SHALL
return the caller's open `waiting` items, soonest due date first, with the
same visibility and snoozes.

#### Scenario: Someone else's inbox
- **WHEN** a member who is not in Sam's inbox lists their attention items
- **THEN** none of the items about Sam's conversations is returned

#### Scenario: Order inside a group
- **WHEN** a member has three Pay items: one due in 5 days, one overdue by 2 days and one with no date
- **THEN** they are listed overdue first, then the one due in 5 days, then the undated one

#### Scenario: Long overdue
- **WHEN** a Do item's due date is 8 days ago
- **THEN** it is not listed, and the conversation's own need is unchanged

#### Scenario: A bill long overdue
- **WHEN** a Pay item's due date is 8 days ago
- **THEN** it is still listed, overdue

#### Scenario: A link that just expired
- **WHEN** a Do item's lapse time passed a minute ago and its conversation's need has not been worked out since
- **THEN** it is not listed

#### Scenario: Still waiting
- **WHEN** a member has a `waiting` item due Thursday and a `nudge` item due 10 days ago
- **THEN** the queue lists the nudge after Pay and before Worth a look, and not the waiting item; `state=waiting` lists the waiting item

## ADDED Requirements

### Requirement: Waits that end
The attention projection SHALL accept a need cleared with cause
`heard_back`, treat it as cleared for every member who can see the
container, and list it among what left the member's list today. A `nudge`
need that comes back as `waiting` (the member wrote to the person) SHALL
update the item's reason, so it leaves the queue without being cleared.

#### Scenario: Ana wrote
- **WHEN** `need_changed` clears a Nudge need with cause `heard_back`
- **THEN** the item leaves every member's list and shows in Done today as "heard back"

#### Scenario: Nudged
- **WHEN** `need_changed` carries the same conversation's need as `waiting` again, due next Wednesday
- **THEN** the item leaves the queue, is listed by `state=waiting`, and is not in Done today
