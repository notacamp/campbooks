# attention Specification

## Purpose
One queue of what needs each member, fed by the modules that own the
records: items that point at a record with a reason, a trust, a due date and
the container that decides who may see them, projected from the owning
modules' events with no personal data, plus each member's own "Later".

## Requirements

### Requirement: Attention scopes
The `attention` module SHALL declare the scopes `attention:read` and
`attention:write`, and every endpoint it adds SHALL require one of them.

#### Scenario: Anonymous caller
- **WHEN** a client calls `GET /api/v1/w/{workspace_id}/attention` without an access token
- **THEN** the response is 401 as `application/problem+json`

#### Scenario: Token without the write scope
- **WHEN** a client with only `attention:read` snoozes an item
- **THEN** the response is 403 as `application/problem+json`

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

### Requirement: Projecting conversation needs
The handler `attention.project_conversation_need` SHALL consume
`conversations.need_changed` v1 and keep one item per conversation: it SHALL
open or update the item when the event carries a need, clear it with the
event's cause when it does not, and apply an event only when its revision is
newer than the item's, also when two deliveries for a conversation with no
item yet run at once (parallel workers, a replay): the write itself SHALL
only ever move the item's revision forward, so the older one, landing
second, changes nothing and announces nothing, while a newer one landing
second on the row the other inserted is announced (a clear included), with
that row's id. A clear that arrives before any open SHALL leave a
tombstone (a cleared item with no reason, at the clear's revision) that no
list shows, so a late open of an older revision cannot reopen it. When the
event's newest inbound message (`latest_message`) differs from the item's,
every snooze of the item SHALL end. The item table SHALL be rebuildable by
replaying that handler from the event log; snoozes SHALL survive a rebuild.

#### Scenario: Deliveries out of order
- **WHEN** revision 3 of a conversation's need is delivered before revision 2
- **THEN** the item reflects revision 3 and revision 2 changes nothing

#### Scenario: A clear before its open
- **WHEN** the clear at revision 2 is delivered before the open at revision 1
- **THEN** the item is cleared at revision 2, the open changes nothing, and nothing reaches Home or Done today

#### Scenario: Any order
- **WHEN** a conversation's opens and clears are delivered in any order
- **THEN** the item ends in the state of the newest revision

#### Scenario: Replay
- **WHEN** the items table is emptied and `attention.project_conversation_need` is replayed from the start of the log
- **THEN** the same items exist in the same states, and a member's snooze still hides its item

#### Scenario: Two deliveries racing for a new item
- **WHEN** revisions 1 and 2 of a conversation with no item are handled at once and revision 2 is written first
- **THEN** the item stays at revision 2, and revision 1 (an open or a clear) changes nothing

#### Scenario: A newer clear landing on a racing open
- **WHEN** the open at revision 1 and the clear at revision 2 of a conversation with no item are handled at once and the open is written first
- **THEN** the item is cleared at revision 2 and the clear is announced with the item's id

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

### Requirement: Later
`POST …/attention/{item_id}/snooze` (`attention.snooze`, `attention:write`)
SHALL hide an item from the caller only, until the given time, which SHALL be
in the future and at most a year ahead (else 400 `invalid_until`).
`attention.unsnooze` SHALL undo it. A newer message from the sender in the
conversation SHALL end the snooze, whether or not it raises a new need. An
item the caller cannot see SHALL answer 404.

#### Scenario: Back on Monday
- **WHEN** a member snoozes an item until Monday 08:00
- **THEN** it is left out of their list until then, other members still see it, and it is listed again from Monday 08:00

#### Scenario: A follow-up
- **WHEN** the sender writes again while the item is snoozed, asking nothing new
- **THEN** the item is listed again at once

### Requirement: Attention in the change feed
`attention.item_changed` v1 SHALL be published when an item opens, changes,
clears, or is snoozed or unsnoozed, carrying ids only, and SHALL declare an
upsert of `attention.item` within the item's container.

#### Scenario: New mail needs a reply
- **WHEN** a message that needs a reply is stored in Ana's inbox
- **THEN** Ana's open change feed receives an upsert of `attention.item`, and a member who cannot see that inbox receives none

### Requirement: Purging a container
The module SHALL offer a way to delete every item (and its snoozes) inside
one container, for when the owning module deletes it (an inbox).

#### Scenario: An inbox goes
- **WHEN** the items of a deleted inbox are purged
- **THEN** none of them is left, and items of other inboxes are kept

### Requirement: Attention workspace deletion
When `workspaces.workspace_deleting` is published, the module SHALL delete the
workspace's items and snoozes when its grace period ends; when
`workspaces.member_left` is published, it SHALL delete that member's snoozes.

#### Scenario: Purge
- **WHEN** a workspace is deleted with its grace period over
- **THEN** its items and snoozes are gone and other workspaces' are kept

### Requirement: Needs cleared by a rule
The attention projection SHALL accept a need cleared with cause `rule` and
treat it as cleared for every member who can see the container, without
listing it among what left the member's list today.

#### Scenario: A rule says never
- **WHEN** `need_changed` clears a Pay need with cause `rule`
- **THEN** the item leaves every member's list and is not shown in Done today

### Requirement: Needs that lapsed
The attention projection SHALL accept a need cleared with cause `lapsed` and
treat it as cleared for every member who can see the container, without
listing it among what left the member's list today.

#### Scenario: A sign-in link
- **WHEN** `need_changed` clears a Do need with cause `lapsed`
- **THEN** the item leaves every member's list and is not shown in Done today

### Requirement: Needs cleared by being opened
The attention projection SHALL accept a need cleared with cause `opened` and
treat it as cleared for every member who can see the container, without
listing it among what left the member's list today. A `worth_a_look` item
SHALL be a reason of its own, listed after Pay.

#### Scenario: Opened
- **WHEN** `need_changed` clears a `worth_a_look` need with cause `opened`
- **THEN** the item leaves every member's list and is not shown in Done today

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
