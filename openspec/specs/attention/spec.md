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
every snooze of the item SHALL end. The item SHALL keep the need's
`warning` (false when absent) and `from_person` (unknown when absent), the
facts its band is worked out from. The item table SHALL be rebuildable by
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

#### Scenario: The facts of a warning
- **WHEN** `need_changed` carries a `review` need with `warning: true`, and an older event carries one with neither `warning` nor `from_person`
- **THEN** the first item is kept as a warning, and the second as no warning with its sender unknown

### Requirement: Listing what needs the caller
`GET /api/v1/w/{workspace_id}/attention` (`attention.list`, `attention:read`)
SHALL return the caller's open items, leaving out items in containers the
caller cannot see (the kernel's `Visibility` port), items the caller snoozed
until a later time, items whose lapse time has passed, `waiting` items, task
items that are not for today (see *Only today's tasks on Home*), and items
other than `pay`, `nudge` and tasks more than 7 days past their due date (a
bill or a nudge stays until a member answers it). Items SHALL be ordered by
priority band (`now`, `today`, `week`, `whenever`, see *Priority bands*),
then inside a band the warning first, then dated before undated and the
soonest date first (a task's planned day dating it when it has no due date),
then items a person sent before the others, then the oldest cause first,
then the item id. Each open item SHALL carry its `band` and its
`band_reason`, and the response SHALL carry the count per reason and
`tasks_elsewhere`. The caller's day (for what is overdue, for the bands and
for Done today) SHALL follow `utc_offset`, the caller's offset from UTC in
minutes (default 0; beyond 14 hours, 400 `invalid_offset`). With
`state=cleared` it SHALL return the items cleared since the start of the
caller's day, newest first, with what cleared them and who, leaving out those
cleared as `superseded`, `rule` or `lapsed`. With `state=waiting` it SHALL
return the caller's open `waiting` items, soonest due date first, with the
same visibility and snoozes. Items of `state=cleared` and `state=waiting`
SHALL carry no band.

#### Scenario: Someone else's inbox
- **WHEN** a member who is not in Sam's inbox lists their attention items
- **THEN** none of the items about Sam's conversations is returned

#### Scenario: Order inside a group
- **WHEN** a member has three Pay items: one due in 5 days, one overdue by 2 days and one with no date, and an undated Review item that arrived earlier than the undated bill
- **THEN** the overdue bill is first (Now), the one due in 5 days next (This week), then the Review item and the undated bill (Whenever, oldest first)

#### Scenario: Long overdue
- **WHEN** a Do item's due date is 8 days ago
- **THEN** it is not listed, and the conversation's own need is unchanged

#### Scenario: A bill long overdue
- **WHEN** a Pay item's due date is 8 days ago
- **THEN** it is still listed, overdue, in Now

#### Scenario: A link that just expired
- **WHEN** a Do item's lapse time passed a minute ago and its conversation's need has not been worked out since
- **THEN** it is not listed

#### Scenario: Still waiting
- **WHEN** a member has a `waiting` item due Thursday and a `nudge` item due 10 days ago
- **THEN** the queue lists the nudge in Today, and not the waiting item; `state=waiting` lists the waiting item, with no band

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

### Requirement: Projecting tasks
The handler `attention.project_task` SHALL consume `tasks.task_changed` and
keep one item per task: reason `do` or `pay` from the task's verb, trust
`sure`, the task's due date, its plan (`none`, `date`, `later`) and planned
day, the assignee's container (visible to that member only), and the task's
source as its cause when it has one. It SHALL clear the item when the task is
done, not needed or handed off, apply newer revisions only, and leave a
tombstone for a clear that arrives before its open.

#### Scenario: A task for Ana
- **WHEN** `task_changed` reports an open `pay` task assigned to Ana, due today
- **THEN** Ana has a `pay` item due today for that task, and João does not see it

#### Scenario: Handed to João
- **WHEN** Ana hands the task to João
- **THEN** Ana's item is cleared and João has one for the same task

#### Scenario: Out of order
- **WHEN** the `done` revision of a task is delivered before its `created` revision
- **THEN** the task has no open item

#### Scenario: Planned for today
- **WHEN** `task_changed` reports a task with the plan `date` on today and no due date
- **THEN** the item keeps the plan and the day

### Requirement: Tasks put off until later
An item for a task planned Later SHALL be left out of `attention.list` until
its Later time, and SHALL come back by itself once that time has passed.

#### Scenario: Monday morning
- **WHEN** a task is planned Later until Monday 09:00
- **THEN** it is not listed on Sunday, and it is listed on Monday at 09:01

### Requirement: A task replaces its conversation's item
`attention.list` SHALL leave out a conversation's `do` or `pay` item for a
member who has a task item from that conversation that is open, or that was
cleared at or after the conversation item's `since`.

#### Scenario: Made a task from the lease email
- **WHEN** Ana makes a task from a conversation that has a Do need
- **THEN** Home lists the task once and does not list the conversation's Do item

#### Scenario: Paid
- **WHEN** Ana marks the task from the EDP invoice paid
- **THEN** neither the task nor the conversation's Pay item is on Home

#### Scenario: A new bill in the same thread
- **WHEN** after Ana paid, a new EDP invoice arrives in that conversation and opens a newer Pay item
- **THEN** Home shows that Pay item again until its own task is made

### Requirement: Priority bands
Each item of the open list SHALL be in one band, by the first rule its facts
meet, and SHALL carry the rule as `band_reason` (`rule`, with `days` or `on`
when the rule names them):

1. the suspicious-mail warning: `now`, `warning`;
2. due before the caller's today (never a nudge): `now`, `overdue` with the
   days late;
3. due today or tomorrow (never a nudge): `today`, `due_today` or
   `due_tomorrow`;
4. a task planned (`date`) for today or a day before: `today`,
   `planned_today`;
5. a nudge: `today`, `nudge_due` with the day an answer was expected;
6. a reply a person sent 3 whole days ago or more: `today`, `waited` with
   the whole days;
7. due within the next 7 days (never a nudge): `week`, `due_on` with the day;
8. a reply or a choice a person sent: `week`, `waiting_answer`;
9. "Worth a look": `whenever`, `kept_by_rule`;
10. anything else: `whenever`, `due_on` with the day when it is dated, else
    `no_date`.

An invitation awaiting an answer (an item about a `schedule.event`) SHALL be
dated by the day its event starts: rule 3 SHALL name it `event_today` or
`event_tomorrow`, rules 7 and 10 `event_on` with the day; it SHALL never be
overdue (a day already begun counts as today) and rule 8 SHALL not apply to
it.

Whether a person sent an item SHALL come from the need's `from_person` (an
invitation's organiser); an item whose sender is unknown SHALL count as not
sent by a person. No model SHALL be asked: the bands come from dates,
reasons and header facts only.

#### Scenario: Four bands
- **WHEN** Sam has a phishing warning, a bill overdue by a day, Pedro's reply waiting for 4 days, a bill due in 2 days, Marta's question from an hour ago and a newsletter kept by a rule
- **THEN** the warning and the bill are in Now, Pedro in Today ("waited", 4 days), the bill due in 2 days and Marta in This week, and the newsletter in Whenever ("kept by rule")

#### Scenario: Exactly three days
- **WHEN** Pedro's reply arrived exactly 3 days ago, and Rita's 3 days less a minute ago
- **THEN** Pedro is in Today and Rita in This week

#### Scenario: A service is never waiting
- **WHEN** a Reply item from a no-reply sender arrived 9 days ago
- **THEN** it is in Whenever with `no_date`

#### Scenario: A nudge is not overdue
- **WHEN** a nudge's expected day was 10 days ago
- **THEN** it is in Today with `nudge_due` on that day, not in Now

#### Scenario: Invitations by the day of their event
- **WHEN** Sam has invitations from people for events today, tomorrow, in 5 days and in 12 days
- **THEN** they are in Today (`event_today`), Today (`event_tomorrow`), This week (`event_on`) and Whenever (`event_on`)

### Requirement: Only today's tasks on Home
The open list SHALL include a task item only when the task is overdue, due
today or tomorrow, planned (`date`) for today or a day before, or back from
Later (its Later time has passed). The other open task items SHALL be left
out and counted in `tasks_elsewhere`. A task item projected before plans were
kept (no plan stored) SHALL be listed as before, until its task changes.

#### Scenario: A week of tasks
- **WHEN** Ana has tasks overdue by 3 days, due today, due tomorrow, planned for today, due in 2 days, undated, and planned for tomorrow
- **THEN** the list holds the first four and `tasks_elsewhere` is 3

#### Scenario: Back from Later
- **WHEN** an undated task's Later time passed a minute ago
- **THEN** it is listed again

### Requirement: Projecting invitations
The handler `attention.project_invitation` SHALL consume
`schedule.invitation_changed` v1 and keep one item per event: while the
event's invitation awaits its member's answer, an item about the event
(`schedule.event`, also its cause) with reason `decide`, trust `sure`, due
on the day the event starts, lapsing when the invitation stops awaiting an
answer, with whether a person organises it, in the member's container
(`schedule.member:<member>`, visible to that member only); once it no longer
does, the item SHALL be cleared with the event's `closed_because`. It SHALL
apply newer revisions only and leave a tombstone for a close that arrives
before its open, announcing a close only over an item that was there.

#### Scenario: João's invitation for Ana
- **WHEN** `invitation_changed` reports an open invitation to Ana for an event tomorrow
- **THEN** Ana has a Decide item for that event, in Today with `event_tomorrow`, counted under `decide`, and João does not see it

#### Scenario: Answered, then taken back
- **WHEN** Ana's invitation is closed as `answered`, then opened again at a newer revision
- **THEN** the item is cleared, then open again with the same id

#### Scenario: The answer before the invitation
- **WHEN** the close at revision 2 is delivered before the open at revision 1
- **THEN** the item stays cleared and no `attention.item_changed` is published

#### Scenario: The event started
- **WHEN** an open invitation's lapse time passed a minute ago and no close has arrived
- **THEN** it is not listed

### Requirement: A task that asks Done? stays on Home
The projection of a task SHALL keep `asks_done` from `tasks.task_changed` (false
when the event has none) with the item, and clear it when the task closes.
`attention.list` SHALL count a task that asks Done? as for today whatever its
plan, so it is listed and not left out under "more in Tasks"; its band follows the
existing rules.

#### Scenario: Planned for next week, then Ana replies
- **WHEN** a task planned nine days ahead is not on Home and `task_changed` says it asks Done?
- **THEN** its item is listed

#### Scenario: Not yet
- **WHEN** a later `task_changed` says it no longer asks
- **THEN** the plan decides again, and an older event replayed changes nothing
