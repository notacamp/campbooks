## ADDED Requirements

### Requirement: Needs cleared by being opened
The attention projection SHALL accept a need cleared with cause `opened` and
treat it as cleared for every member who can see the container, without
listing it among what left the member's list today. A `worth_a_look` item
SHALL be a reason of its own, listed after Pay.

#### Scenario: Opened
- **WHEN** `need_changed` clears a `worth_a_look` need with cause `opened`
- **THEN** the item leaves every member's list and is not shown in Done today

## MODIFIED Requirements

### Requirement: Listing what needs the caller
`GET /api/v1/w/{workspace_id}/attention` (`attention.list`, `attention:read`)
SHALL return the caller's open items, leaving out items in containers the
caller cannot see (the kernel's `Visibility` port), items the caller snoozed
until a later time, and items more than 7 days past their due date. Items
SHALL be ordered by reason (`review`, `reply`, `decide`, `do`, `pay`, `worth_a_look`), then
overdue first, then by due date, then urgent first, then oldest cause first,
and the response SHALL carry the count per reason. The caller's day (for what
is overdue and for Done today) SHALL follow `utc_offset`, the caller's offset
from UTC in minutes (default 0; beyond 14 hours, 400 `invalid_offset`). With
`state=cleared` it SHALL return the items cleared since the start of the
caller's day, newest first, with what cleared them and who.

#### Scenario: Someone else's inbox
- **WHEN** a member who is not in Sam's inbox lists their attention items
- **THEN** none of the items about Sam's conversations is returned

#### Scenario: Order inside a group
- **WHEN** a member has three Pay items: one due in 5 days, one overdue by 2 days and one with no date
- **THEN** they are listed overdue first, then the one due in 5 days, then the undated one

#### Scenario: Long overdue
- **WHEN** an item's due date is 8 days ago
- **THEN** it is not listed, and the conversation's own need is unchanged
