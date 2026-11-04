## MODIFIED Requirements

### Requirement: The send outcome
On `connectors.send_completed` v1
(`contracts/connectors.send_completed.v1.schema.json`), the consumer
`conversations.record_send` SHALL delete a sent draft, releasing its blobs,
and publish `conversations.draft_sent`; or mark a failed draft `failed` with
the reason and publish `conversations.send_failed`. A failed draft SHALL be
editable and may be sent again. The sent message SHALL reach its
conversation through the connector's change batch, like any message. A
released send that hears nothing back for an hour SHALL fail with
`unknown_outcome`, and a late outcome for it SHALL still be recorded.
The composer SHALL say a failed draft's reason in words; for
`attachment_refused` it SHALL say the mail provider would not accept one
of the attachments and nothing was sent, never to check the addresses.

#### Scenario: The reply appears in its conversation
- **WHEN** a reply is sent
- **THEN** the conversation gains one outbound message and the draft is gone

#### Scenario: Gmail refuses the message
- **WHEN** the connector reports `rejected`
- **THEN** the draft is `failed` with reason `rejected` and can be edited and sent again

#### Scenario: The provider refuses an attachment
- **WHEN** the connector reports `attachment_refused`
- **THEN** the draft is `failed` with reason `attachment_refused`, and the composer says one of the attachments was not accepted and nothing was sent
