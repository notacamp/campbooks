# Tasks

## 1. The kernel

- [x] 1.1 `outbound::{Destination, Resolved}`, the `OutboundDestinations` port (`NoDestinations` until registered) and `AppState::with_destinations`
- [x] 1.2 `OutboundRequests::stored_send` takes the destination the sender resolved

## 2. Sends (conversations)

- [x] 2.1 `binding::Bound` and the `…/v2` encodings of `SendRequest` and `StoredSend` with the destination; `holds_at` / `retag_at` accept `…/v1` only for the destination recorded at the deploy
- [x] 2.2 `send_draft` signs the destination; `release_send` verifies it against the destination resolved again and signs the stored send for it
- [x] 2.3 The outbound-requests port verifies the stored send for the connector's destination; the rotation re-tag resolves the destination too

## 3. The connector

- [x] 3.1 `domain::destinations`: the port, `of_inbox` (one stream only), `resolved`, `as_at_deploy`; registered by the server
- [x] 3.2 `queue_send` and every `send_message` attempt pass the destination they would send through
- [x] 3.3 Outbound changes `…/v2`: bound by `request_changes` to the stream's connection and mailbox, verified by each run for the ones it loaded; `…/v1` only as at the deploy
- [x] 3.4 Zoho's `restore_to` signed into the trash change by the run that made it; an untrash whose place does not verify is refused; the rotation re-tag covers it

## 4. The deploy

- [x] 4.1 Migration `20261023100000`: `connectors.destinations_at_deploy` and `connectors.restores_at_deploy`, read-only for the runtime role; the server's grant test lists them

## 5. Tests

- [x] 5.1 Unit: the destination in both send MACs (another connection, stream, mailbox or none does not verify); `…/v1` only as at the deploy; a v2 tag is never a v1 one
- [x] 5.2 Real ports: a scheduled send is released through its stream; repointed after the member asked, it is never released
- [x] 5.3 Connector: a send repointed after its release, or before it was recorded, reaches no provider and fails `unavailable` without a retry; released where it goes, it is sent
- [x] 5.4 Connector: a change whose stream was repointed after it was recorded ends `skipped` (`unverified`); a `…/v1` change is made only where its stream went at the deploy
- [x] 5.5 Zoho: a `restore_to` edited by SQL is never used; trash and back still restores to the member's folder
- [x] 5.6 The port: a stored send stands only for its destination; a `…/v1` one only as at the deploy
