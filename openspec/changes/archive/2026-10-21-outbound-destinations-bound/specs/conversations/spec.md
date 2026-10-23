## ADDED Requirements

### Requirement: Sends are bound to where they go
Both send MACs SHALL name the destination the inbox's mail goes out through
(the connection, stream and mailbox the connector's outbound-destinations
port answers; or none), under the labels
`campbooks/conversations.send-request/v2` and
`campbooks/conversations.stored-send/v2`. `send_draft` SHALL sign the
destination as resolved when the member asks; `release_send` SHALL resolve
it again and release only when the author's request verifies for it (a
draft that does not is left as it is, with a WARN line carrying ids only,
and no retry), then sign the stored send for the same destination. The
outbound-requests port SHALL say a send stands only when its stored-send MAC
verifies for the destination the connector passes. A MAC of the earlier
encoding (`…/v1`, signed before destinations were bound) SHALL verify only
when the connector says the destination is the one recorded when
destinations were first bound.

#### Scenario: A scheduled send released where its member sent it
- **WHEN** Sam schedules a send from an inbox a stream feeds and its window ends
- **THEN** it is released and the connector records it through that stream

#### Scenario: A stream repointed after the member asked
- **WHEN** SQL points the inbox's stream at another mailbox before the release
- **THEN** nothing is released, no `conversations.send_requested` is published, the connector records nothing and the draft is left scheduled

#### Scenario: A stored send asked about for another destination
- **WHEN** the connector asks about a released send for a destination other than the one it was released to
- **THEN** the port says it does not stand

#### Scenario: A send released before destinations were bound
- **WHEN** a send released by the earlier version is asked about
- **THEN** it stands only when the connector says its destination is the one recorded at the deploy
