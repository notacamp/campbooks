## ADDED Requirements

### Requirement: The calendar list is read again
For each connection that holds the `calendar` capability, the calendar stream
SHALL read the provider's calendar list again about once a day, jittered, and
SHALL publish `connectors.calendar_list_read` v1 with the same `calendar-list`
blob as a grant. The event SHALL carry ids and a blob reference only, never a
provider calendar id. `connectors.capability_granted` SHALL be published only
for a grant. The stream SHALL remember a calendar it did not know as not
syncing. A connection without the `calendar` capability SHALL NOT have its
calendar list read. A provider error on this read SHALL NOT stop the stream's
event sync, SHALL NOT change the stream's health or the connection's, and
SHALL be tried again within the hour.

#### Scenario: A calendar shared later
- **WHEN** Marta shares her calendar with Inês a week after Inês connected calendar access, and the daily read runs
- **THEN** `calendar_list_read` carries a list that includes Marta's calendar, the stream knows it as not syncing, and no `capability_granted` is published

#### Scenario: No calendar access
- **WHEN** a connection has only the `messages` capability
- **THEN** no calendar list is read for it and no `calendar_list_read` is published

#### Scenario: Once a day
- **WHEN** the list was read an hour ago and the stream runs again
- **THEN** the list is not read again

#### Scenario: The provider fails the list
- **WHEN** Google answers an error to the calendar list while the events can be read
- **THEN** events are read and applied as usual, the stream's health reason stays empty, and the list is asked for again within the hour
