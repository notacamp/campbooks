## ADDED Requirements

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

## MODIFIED Requirements

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
