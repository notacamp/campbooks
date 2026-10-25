## MODIFIED Requirements

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

## ADDED Requirements

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

Whether a person sent an item SHALL come from the need's `from_person`; an
item whose sender is unknown SHALL count as not sent by a person. No model
SHALL be asked: the bands come from dates, reasons and header facts only.

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
