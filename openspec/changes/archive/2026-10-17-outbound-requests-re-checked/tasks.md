# Tasks

## 1. Inventory

- [x] 1.1 List every connectors handler that acts at the provider because of an event, with its trigger and action (`design.md`)

## 2. The port

- [x] 2.1 `kernel::outbound::OutboundRequests` (`stored_send`, `changes_standing`), `NoOutboundRequests`, `AppState::with_outbound`
- [x] 2.2 `conversations` implements it (`outbound_requests`); the server registers it in `with_ports`
- [x] 2.3 `release_send` records the document with the send (`drafts.send_document`, migration `20261021190000`); `author_may_send` shared with the port

## 3. Re-check before acting

- [x] 3.1 `queue_send` records only a send that stands, from the event's inbox, with the port's document
- [x] 3.2 `send_message` asks again before any provider call; sends the port's document, equal to the recorded one; settles a send that does not stand
- [x] 3.3 `request_changes` records only the changes that stand, before asking for a history stamp

## 4. Tests

- [x] 4.1 Connectors, fake Gmail: a forged send for no released send, for another inbox, with other content; a send that stops standing before its call; a recorded send whose document differs; a replay after the send
- [x] 4.2 Connectors, fake Gmail: a forged change request changes nothing; only the changes a conversation shows are made
- [x] 4.3 Conversations: the port's answers for a released send, a gone author, a failed draft, a draft with no document, and for each change kind, undone and forged
- [x] 4.4 Server: the real port confirms a legitimate release and refuses a forged one
