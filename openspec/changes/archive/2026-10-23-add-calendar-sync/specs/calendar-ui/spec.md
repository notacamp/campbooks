## Purpose

The Calendar screens' parts that need provider calendars: the sheet of a
Google or Zoho event, Add to my calendar, and calendar access in Settings.
Builds on the Calendar place of `add-schedule-module`.

## ADDED Requirements

### Requirement: Provider events in the event sheet
For an event on a provider calendar the sheet SHALL offer Accept / Maybe /
Decline for an invitation. For an event on the Campbooks calendar it SHALL
offer Add to my calendar, shown only when the member has a provider calendar
that can be written, with a list of those calendars, the last used first. An
event whose change was not kept SHALL say so ("Changed in Google meanwhile;
we kept theirs"). A pending change SHALL show as pending. The reader's strip
SHALL name the calendar an event was pushed to.

#### Scenario: A change not kept
- **WHEN** Google refused the user's move because the event changed there
- **THEN** the sheet shows the provider's time and "Changed in Google meanwhile; we kept theirs"

#### Scenario: Adding to a calendar
- **WHEN** the user opens Scout's concert event and has a writable Google calendar
- **THEN** the sheet offers Add to my calendar with that calendar listed

### Requirement: Calendar access and provider calendars in Settings
Settings → Calendar SHALL also list the member's provider calendars with sync
on or off, offer "Also add to" per kind, and, for each mail connection
without calendar access, "Add calendar access", which starts the connection's
consent for the `calendar` capability. "Add calendar access" SHALL show only
while connectors offers the capability.

#### Scenario: Adding calendar access
- **WHEN** the user presses "Add calendar access" next to their Gmail connection
- **THEN** the provider's consent opens for calendar access on that connection, and after it the Google calendars are listed
