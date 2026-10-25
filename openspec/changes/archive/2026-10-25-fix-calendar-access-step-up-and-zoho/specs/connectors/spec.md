## ADDED Requirements

### Requirement: A connection says what it could still be given
The connection view (`connectors.get_connection`, `connectors.list_connections`
and the answers that return one) SHALL carry `offers`: the capabilities the
connection could be given by consenting again and does not hold yet. A Google
connection SHALL offer `calendar`; a Zoho connection SHALL offer `calendar`
only while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on and Zoho is configured;
Microsoft and IMAP connections SHALL offer nothing; a capability the
connection holds SHALL never be in it.

#### Scenario: Google offers calendar
- **WHEN** a Google connection holds `messages` only
- **THEN** its `offers` is `["calendar"]`, and once it holds `calendar` it is empty

#### Scenario: Zoho follows its flag
- **WHEN** a Zoho connection holds `messages` only
- **THEN** its `offers` is empty while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is off and `["calendar"]` while it is on

#### Scenario: Microsoft and IMAP
- **WHEN** a Microsoft or an IMAP connection is read
- **THEN** its `offers` is empty
