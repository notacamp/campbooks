## MODIFIED Requirements

### Requirement: Provider calendars
When `connectors.capability_granted` is published for the `calendar`
capability, the module SHALL create one calendar per provider calendar the
grant lists, owned by the granting member, with the provider's name, colour,
time zone, whether it may be written and whether the account owns it; a
calendar the account does not own SHALL be following and any other mine, and
the provider's primary calendar SHALL sync while the others SHALL be off
until the member turns them on. A repeated grant SHALL add only calendars
the module does not have yet, and SHALL record whether the account owns a
calendar the module has when it did not know, changing nothing else.
When `connectors.calendar_list_read` is published, the module SHALL apply it
by the same rule as a repeated grant, except that a calendar it does not have
yet SHALL be added off whatever the provider says is primary, and a calendar
SHALL never be turned on by it. Nothing but `owned` SHALL change on a calendar
the module has. It SHALL NOT make the first calendar of a connection for a
member: when the module holds none (the connection was removed, or the member
left), a late read adds nothing.
Turning a calendar off SHALL hide its events and stop applying its changes;
turning it on SHALL ask connectors to read it again from the start
(`schedule.calendar_sync_changed`).

#### Scenario: A calendar grant
- **WHEN** Inês grants calendar access on her Google connection, which lists "Inês" (primary, owned), "Family" (owned) and "Holidays in Portugal" (not owned)
- **THEN** she has four calendars: her Campbooks calendar, "Inês" mine and syncing, "Family" off and mine when turned on, and "Holidays in Portugal" off and following when turned on

#### Scenario: The same grant again
- **WHEN** the same calendar grant is published again
- **THEN** no calendar is added or changed

#### Scenario: A grant that knows more
- **WHEN** Inês reconnects her Google calendar access and the list now says she does not own "Holidays in Portugal", which the module had with ownership unknown and set to mine
- **THEN** "Holidays in Portugal" shows as not owned and stays mine

#### Scenario: Turning a calendar on
- **WHEN** Inês turns "Family" on
- **THEN** connectors reads "Family" from the start and its events appear once the batch is applied

#### Scenario: A calendar shared later
- **WHEN** a daily read lists "Marta" (not owned) that Inês did not have
- **THEN** Inês has "Marta" off and following when turned on, and her other calendars are as they were

#### Scenario: A calendar she made later
- **WHEN** a daily read lists "Side project" (owned) that Inês did not have
- **THEN** she has it off and mine when turned on

#### Scenario: Nothing she chose is undone
- **WHEN** a daily read lists "Family", which Inês set to mine, as not owned and renamed at the provider
- **THEN** "Family" stays mine with its name and colour, and only its `owned` is filled if it was unknown

#### Scenario: A read that arrives late
- **WHEN** a daily read arrives after the connection was removed, or after the member left the workspace
- **THEN** no calendar is added
