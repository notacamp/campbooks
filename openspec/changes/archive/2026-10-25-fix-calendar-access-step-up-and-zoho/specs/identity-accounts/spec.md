## ADDED Requirements

### Requirement: A reconnect can ask for calendar access
`identity.start_link` SHALL accept `capabilities`, of which only `calendar` is
allowed (400 `link_incomplete` for any other). The consent SHALL ask besides
the mailbox for the scopes of those capabilities and, for a `reconnect`, of
those the connection already holds. Zoho's calendar scopes SHALL still be
asked only while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on. The recent
sign-in rule of a reconnect is unchanged.

#### Scenario: Adding calendar to a Google connection
- **WHEN** a member starts a `reconnect` with `capabilities: ["calendar"]` for a Google connection that has mail only
- **THEN** the consent at Google asks for the mailbox scopes and the calendar scopes

#### Scenario: A capability that is not offered
- **WHEN** a member starts a `reconnect` with `capabilities: ["files"]`
- **THEN** the response is 400 and no ticket exists
