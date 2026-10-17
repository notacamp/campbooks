# Tasks

## 1. Inventory

- [x] 1.1 List every connectors handler that acts at the provider because of an event, with its trigger and action (`design.md`)

## 2. The port

- [x] 2.1 `kernel::outbound::OutboundRequests` (`stored_send`, `changes_standing`), `NoOutboundRequests`, `AppState::with_outbound`
- [x] 2.2 `conversations` implements it (`outbound_requests`); the server registers it in `with_ports`
- [x] 2.3 `release_send` records the document with the send (`drafts.send_document`, migration `20261021200000`); `author_may_send` shared with the port
- [x] 2.4 `conversation::request` records every change request under its event id (`change_requests`, migration `20261021200100`)

## 3. Re-check before acting

- [x] 3.1 `queue_send` records only a send that stands, from the event's inbox, with the port's document
- [x] 3.2 `send_message` asks again before any provider call; sends only while the port confirms the recorded document; looks an unclear attempt up first; settles a send that does not stand
- [x] 3.3 `request_changes` records only the changes recorded under that request, before asking for a history stamp
- [x] 3.4 `send_message` claims each attempt atomically and reads only a stream of the send's own inbox

## 4. Tests

- [x] 4.1 Connectors, fake Gmail: a forged send for no released send, for another inbox, with other content; a send that stops standing before its call; a recorded send whose document differs; a replay after the send; a send row pointed at another inbox's stream; an attempt claimed once; two jobs of one send
- [x] 4.2 Connectors, fake Gmail: a forged change request changes nothing; only the changes a conversation shows are made
- [x] 4.3 Conversations: the port's answers for a released send, a gone author, a failed draft, a draft with no document; for a recorded change request, undone, smuggled, rekeyed and forged
- [x] 4.4 Server: the real port confirms a legitimate release and change, and refuses a forged send and a forged change request
