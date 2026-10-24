## ADDED Requirements

### Requirement: Calendar access is offered
A member SHALL be able to add the `calendar` capability to a Google or Zoho
connection of theirs through the same front door and extend path as any
capability, so the connection gains it with no second connection. Google
calendar access SHALL ask for
`https://www.googleapis.com/auth/calendar.events` and
`https://www.googleapis.com/auth/calendar.calendarlist.readonly`; Zoho
calendar access SHALL ask for `ZohoCalendar.calendar.READ` and
`ZohoCalendar.event.ALL`, and SHALL be offered only while
`CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on (off by default). Signing in
SHALL keep asking for mail only. When the grant comes back, the module SHALL
list the account's calendars, publish `connectors.capability_granted` for
`calendar` with a blob listing them (provider id, name, colour, time zone,
primary, may be written), and create a calendar stream due now. A provider
that refuses the calendar API (not enabled on the client's project) SHALL
leave the connection's mail as it was and mark the calendar stream's health
with a reason a member can act on.

#### Scenario: Adding calendar to Gmail
- **WHEN** Inês adds calendar access to her Google connection
- **THEN** the consent asks for the two calendar scopes, the same connection gains `calendar`, `capability_granted` lists her calendars, and a calendar stream is due now

#### Scenario: Zoho calendar switched off
- **WHEN** `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is off and a member asks for calendar access on a Zoho connection
- **THEN** the answer is 409 `capability_not_offered` and nothing changes

#### Scenario: Calendar API not enabled
- **WHEN** Google answers 403 `accessNotConfigured` to the calendar list
- **THEN** mail keeps syncing and the calendar stream's health says the calendar API is not enabled for this server

### Requirement: Google Calendar adapter
The Google Calendar adapter SHALL read each calendar schedule asks it to
sync: first the events that end 90 days ago or later and every series still
recurring, with series returned once (not expanded into instances) and each
exception as its own event, recording the calendar's sync token; then changes
incrementally from that token. A token Google no longer accepts (410) SHALL
trigger a full read of that calendar that emits upserts for new or changed
events and removals for events it had that are gone. Each event SHALL be
normalised to the calendar change contract (provider id, etag, status,
start and end with time zone or all-day dates, recurrence rules, the series
and original start of an exception, title, place, description, organiser,
attendees and the member's response, and Campbooks' private properties
when present), with the text stored as a blob before the batch that
references it. Requests SHALL take from the provider's shared budget and
follow the run's backoff, throttling and health rules.

#### Scenario: First read then incremental
- **WHEN** a calendar stream runs for the first time, then again after Inês adds an event in Google
- **THEN** the first batch carries the calendar's events and series, and the second only the new event

#### Scenario: Expired token
- **WHEN** Google answers 410 to a sync token
- **THEN** the calendar is read in full, and events that disappeared are emitted as removals

### Requirement: Zoho Calendar adapter
While `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on, the Zoho Calendar adapter
SHALL read each calendar schedule asks it to sync by date range in the
account's data centre, keep its own change marker in the stream's cursor,
and normalise events to the same calendar change contract. Events Campbooks
wrote SHALL be recognised by the provider id the module stored when it wrote
them, since Zoho keeps no private properties.

#### Scenario: A Zoho event
- **WHEN** a Zoho calendar holds "Accountant, Av. da Liberdade" on Thursday at 15:00
- **THEN** a batch carries it in the same shape as a Google event

### Requirement: The calendar change stream
The module SHALL publish `connectors.calendar_changed` v1 per calendar with
the batch's upserts and removals, ids and blob references only, in the
transaction that advances the calendar's cursor, so a batch is never lost or
applied twice. Batches for one calendar SHALL be published in order. A
calendar schedule does not sync SHALL not be read.

#### Scenario: Cursor and event together
- **WHEN** a run reads three changed events and the transaction commits
- **THEN** one `calendar_changed` batch with three upserts is published and the cursor moves; when it does not commit, neither happens

### Requirement: Outbound changes to calendars
The module SHALL consume `schedule.outbound_requested` v1 and, after the
schedule module confirms the request through the outbound-requests port,
record the change durably, bound to that request with a MAC as mail changes
are, before it is attempted; a change whose MAC does not verify SHALL never
be made. The calendar stream's next run SHALL make recorded changes before
it reads: create, update one event or one occurrence, delete, or the
member's response to an invitation. An update or delete SHALL send
`If-Match` with the etag the request names; a 412 SHALL end the change
`superseded` (`changed_at_provider`) and re-read the event, so the
provider's version reaches schedule. Events Campbooks creates in Google
SHALL carry private properties with the Campbooks event id, kind and source
link. Each change SHALL end `applied` (with the provider's new id and etag),
`superseded`, `skipped` or `failed` with a stable reason code and no event
text, and the outcome SHALL be published for schedule. Provider trouble
SHALL keep changes pending under the run's backoff, and a change that met
trouble eight times SHALL be `failed`.

#### Scenario: A move reaches Google
- **WHEN** Inês moves "Dentist" to 16:00 and the stream runs
- **THEN** Google's event moves with `If-Match` her etag, the change is `applied`, and schedule gets the new etag

#### Scenario: Changed at Google first
- **WHEN** the clinic moved the event in Google before the run
- **THEN** Google answers 412, the change is `superseded` (`changed_at_provider`), and the next batch carries Google's version

#### Scenario: A change planted with SQL
- **WHEN** a pending calendar change no member asked for is inserted in the database
- **THEN** the run writes nothing to the provider and the change ends `skipped` (`unverified`)

### Requirement: Calendar write-backs never echo
Changes that came from the provider SHALL never be written back. After a
write, the adapter SHALL hold the etag it got, so the provider's own report
of that write produces no change in the next batch. While a member's change
is pending, a run that emits that event SHALL emit it with the change
applied.

#### Scenario: After a write
- **WHEN** a member's create has been written to Google and the stream runs again
- **THEN** no `calendar_changed` batch carries that event unless Google changed it since

### Requirement: The consent asks for what the connection has
A consent that adds the `calendar` capability to a connection, or connects
one with it, SHALL ask the provider for the calendar scopes besides the
mailbox's, and a reconnect of a connection SHALL ask again for the scopes of
every capability that connection has, so the new grant loses none. A member
who leaves a calendar scope out at the provider's consent still connects their
mailbox: the connection then gains only the capabilities whose scopes were
granted.

#### Scenario: Adding calendar to a Google connection
- **WHEN** Inês asks for calendar access on her Google connection
- **THEN** the consent asks for the mailbox scope and the two calendar scopes, and the connection gains `calendar` when all are granted

#### Scenario: Reconnecting after the grant stopped working
- **WHEN** Inês reconnects a Google connection that has `calendar`
- **THEN** the consent asks for the calendar scopes again, so the connection keeps its calendar

#### Scenario: A calendar scope left out
- **WHEN** Inês grants the mailbox scope and one of the two calendar scopes
- **THEN** the connection gains `messages` only

### Requirement: Calendars are read when the member turns them on
The module SHALL consume `schedule.calendar_sync_changed` v1 and read a
provider calendar only while the member has it on. A calendar turned on again
SHALL be read from the start (its sync token is dropped); one turned off SHALL
not be read. Which calendar of the provider the event is about SHALL travel in
a `calendar-ref` v1 blob, since the provider's id can be an address and never
travels in an event. Deliveries can arrive in any order and more than once:
the newest event a calendar heard SHALL win, and an older or repeated one
SHALL change nothing.

#### Scenario: Turning a calendar on
- **WHEN** Inês turns on her "Holidays" calendar
- **THEN** its next read is a full one, and a batch for it is announced

#### Scenario: Turning a calendar off
- **WHEN** Inês turns it off again
- **THEN** the stream stops reading it

#### Scenario: Off and on, delivered out of order
- **WHEN** Inês turns "Holidays" off and straight back on, and the "on" is delivered first
- **THEN** the late "off" changes nothing, and the calendar is read from the start

### Requirement: A calendar stream says why it does not work
A calendar stream that cannot work SHALL carry a stable reason a member or an
operator can act on (`calendar_api_not_enabled`, `provider_errors`), shown on
the connection's stream, and SHALL leave the connection's health and its mail
streams as they are. Throttling and failures on the calendar stream SHALL slow
that stream only; a refused token or grant, which the mailbox shares, SHALL
follow the connection's rules. A reconnect clears the reason.

#### Scenario: Failures in a row
- **WHEN** the calendar stream fails as many times in a row as the connection's mail would take to be marked failing
- **THEN** the stream's reason is `provider_errors` and the connection's health stays `ok`
