## MODIFIED Requirements

### Requirement: Items are references
An attention item SHALL hold the record it is about, the container that
decides who may see it, the record that caused it, a reason (`review`,
`reply`, `decide`, `do` or `pay`), a trust (`sure` or `check`), an optional due
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

### Requirement: Listing what needs the caller
`GET /api/v1/w/{workspace_id}/attention` (`attention.list`, `attention:read`)
SHALL return the caller's open items, leaving out items in containers the
caller cannot see (the kernel's `Visibility` port), items the caller snoozed
until a later time, items whose lapse time has passed, and items other than
`pay` more than 7 days past their due date (a bill stays until a member
answers it). Items
SHALL be ordered by reason (`review`, `reply`, `decide`, `do`, `pay`), then
overdue first, then by due date, then urgent first, then oldest cause first,
and the response SHALL carry the count per reason. The caller's day (for what
is overdue and for Done today) SHALL follow `utc_offset`, the caller's offset
from UTC in minutes (default 0; beyond 14 hours, 400 `invalid_offset`). With
`state=cleared` it SHALL return the items cleared since the start of the
caller's day, newest first, with what cleared them and who, leaving out those
cleared as `superseded`, `rule` or `lapsed`.

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

## ADDED Requirements

### Requirement: Needs that lapsed
The attention projection SHALL accept a need cleared with cause `lapsed` and
treat it as cleared for every member who can see the container, without
listing it among what left the member's list today.

#### Scenario: A sign-in link
- **WHEN** `need_changed` clears a Do need with cause `lapsed`
- **THEN** the item leaves every member's list and is not shown in Done today
