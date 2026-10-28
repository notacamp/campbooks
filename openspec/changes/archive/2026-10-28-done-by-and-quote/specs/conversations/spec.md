## ADDED Requirements

### Requirement: How an ask gets done
Each ask of the read SHALL carry `done_by`, required by the strict schema: `reply` when replying to the message is itself how the obligation is done (giving an answer, an approval, comments, a confirmation, a choice, information asked for), `elsewhere` when it is done some other way (paying, signing or sending a document or a form, filing, booking, going somewhere, doing work). The instruction SHALL say it in one sentence with contrasting examples. It SHALL be part of the same single call. Code SHALL only veto: a value outside the set SHALL be kept as none, and an ask with verb `pay` SHALL be `elsewhere`. The read SHALL store it with the ask and `conversations.get_message_read` SHALL return it; a read made before it was asked has none.

#### Scenario: Confirm a day
- **WHEN** the email asks "can you confirm Thursday works?"
- **THEN** the ask's `done_by` is `reply`

#### Scenario: Sign and send
- **WHEN** the email asks to sign and return a contract
- **THEN** the ask's `done_by` is `elsewhere`

#### Scenario: A bill
- **WHEN** the model calls a `pay` ask `reply`
- **THEN** it is kept as `elsewhere`

#### Scenario: The sender's sentence stays under the conversation's access
- **WHEN** a member who may not read the inbox asks for the read of a message
- **THEN** the answer is 404 and carries no sentence of the sender's, and `message_read` never does

## MODIFIED Requirements

### Requirement: A read is published
When a read is stored, of either layer, the module SHALL publish
`conversations.message_read` v1 in the same transaction with the message,
conversation and inbox ids, the inbox's personal owner (none for a shared
inbox), the layer, the items (key, kind, start, end, all-day, time zone,
updates or cancels), the asks (key, verb, due, whether it has an amount, same
as, and `done_by` when read) and whether a summary is ready, and no title, place, quote, amount or
other text (`done_by` is an enum). The titles, places and calendar `UID`s of the items SHALL travel
in a `read-texts` v1 blob that the event points at (`texts`) and that the
message holds, so they are deleted with it and are never in the event log.
Item and ask keys SHALL be stable for the same message and position, so a
repeated delivery changes nothing for a consumer.

#### Scenario: The payload of a booking
- **WHEN** a restaurant booking is read
- **THEN** `message_read` carries a `reservation` item with its time and key, and not the restaurant's name
