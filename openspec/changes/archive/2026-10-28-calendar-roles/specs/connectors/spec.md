## MODIFIED Requirements

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
primary, may be written, and whether the account owns it when the provider
says: Google's `accessRole` is `owner`, Zoho's `privilege` is `owner`), and
create a calendar stream due now. A provider that refuses the calendar API
(not enabled on the client's project) SHALL leave the connection's mail as it
was and mark the calendar stream's health with a reason a member can act on.

#### Scenario: Adding calendar to Gmail
- **WHEN** Inês adds calendar access to her Google connection
- **THEN** the consent asks for the two calendar scopes, the same connection gains `calendar`, `capability_granted` lists her calendars, and a calendar stream is due now

#### Scenario: Whose calendars they are
- **WHEN** Inês's Google calendar list has "Inês" (`accessRole` owner), "Marta" (writer) and "Holidays in Portugal" (reader)
- **THEN** the listed blob says "Inês" is owned and "Marta" and "Holidays in Portugal" are not, and "Inês" and "Marta" may be written

#### Scenario: Zoho calendar switched off
- **WHEN** `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is off and a member asks for calendar access on a Zoho connection
- **THEN** the answer is 409 `capability_not_offered` and nothing changes

#### Scenario: Calendar API not enabled
- **WHEN** Google answers 403 `accessNotConfigured` to the calendar list
- **THEN** mail keeps syncing and the calendar stream's health says the calendar API is not enabled for this server
