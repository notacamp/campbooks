- **Tracked events, the record of every action.** Archiving a conversation and putting it
  back are now recorded with who did it (a person, an app, Scout or an automation on their
  behalf), the operation, the request's correlation id and a snapshot of the conversation
  afterwards: ids, its stream, counts and times, never a subject, a name or an address. The
  events are defined in a tracking plan, `tracking/tracking-plan.xlsx`, a workbook anyone can
  open in Excel or LibreOffice; CI checks it. Self-hosters: nothing is recorded unless
  `CAMPBOOKS_TRACKING__SINK=outbox` is set, which writes the events to a table in the
  database (publishing them to a broker comes next).
