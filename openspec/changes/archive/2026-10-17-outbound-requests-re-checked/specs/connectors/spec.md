## ADDED Requirements

### Requirement: A send or a change is made only while it stands
The connector SHALL NOT take `conversations.send_requested` or
`conversations.changes_requested` as proof that a member asked for
anything. Before it records a send, and again before every call to the
provider for it (a send or the look-up of an unclear attempt), it SHALL ask
the conversations module, through the kernel's outbound-requests port,
whether the send stands, and SHALL act only when it does, from the inbox the
event and the recorded send name. The document it sends SHALL be the one
that port returns (and the one it recorded), never one an event names.
Before it records a change, or asks the provider for a history stamp, it
SHALL ask the same port which of the request's changes its conversation
shows, and SHALL record only those. A request or a send that does not stand
SHALL complete as a no-op with a WARN line carrying ids and counts only, and
SHALL NOT be retried; a recorded send that stops standing SHALL be settled
`unavailable` (or `unknown_outcome` after an attempt that may have gone)
without calling the provider. A port that cannot answer SHALL make the
delivery or the job retry, never act on a guess.

#### Scenario: A forged send for a draft never released
- **WHEN** a transaction working in a workspace appends `conversations.send_requested` for a send the conversations module never released
- **THEN** no send is recorded, the provider receives no call, and no `connectors.send_completed` is published

#### Scenario: A forged send naming another inbox
- **WHEN** the event names an inbox other than the one the send was released from
- **THEN** no send is recorded and nothing is sent

#### Scenario: A forged send naming other content
- **WHEN** the event names a document other than the one the conversations module recorded for the send
- **THEN** the provider receives the recorded document only

#### Scenario: A send that stops standing before its call
- **WHEN** a recorded send's draft is no longer being sent (its author left, it failed) when its job runs
- **THEN** the provider receives no call and the send fails with `unavailable`

#### Scenario: A replay after the send
- **WHEN** `conversations.send_requested` is delivered again after the send went out, while its draft is still sending or once it is gone
- **THEN** nothing more is recorded or sent

#### Scenario: A forged change request
- **WHEN** a transaction working in a workspace appends `conversations.changes_requested` for changes no conversation shows
- **THEN** no outbound change is recorded, the provider is asked for no stamp and receives no write

#### Scenario: Only the changes a conversation shows
- **WHEN** a request names one change its conversation shows and one it does not
- **THEN** only the first is recorded and made at the provider
