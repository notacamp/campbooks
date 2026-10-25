# Tasks

## 1. Graph client
- [ ] 1.1 A message's state with its `@odata.etag` / `changeKey`; `PATCH` with `If-Match` (412 / 409 a conflict, 404 gone)
- [ ] 1.2 `POST …/move` with immutable ids, its answer's id only; no call that deletes or empties anything
- [ ] 1.3 `POST /me/sendMail` with base64 MIME (202, 413 `too_large`), the Sent Items copy by `internetMessageId`

## 2. Write-back
- [ ] 2.1 `outbound::map_microsoft`, `microsoft::write::writable`; `request_changes` records Microsoft changes (skipped while Microsoft mail is off)
- [ ] 2.2 `state::view`: the index's tokens as the plan's
- [ ] 2.3 `microsoft/write.rs`: the plan, read-before-write, the conflict rule, patch, move, streams (folder first, else category), where a deleted message was, at most 50 messages a run
- [ ] 2.4 `sync_stream`: the push before the read, charged to the connection's budget; moved ids looked up

## 3. Reader
- [ ] 3.1 Pending changes shown on emissions; `reemit` honoured

## 4. Sending
- [ ] 4.1 `send_message/via_microsoft.rs`: From the verified address only, the size limit, the claim, sendMail, the Sent Items copy indexed; unclear → lookup → `unknown_outcome`

## 5. Tests
- [ ] 5.1 Fake Graph: etag/`If-Match`, patch, move, sendMail into Sent Items, the `internetMessageId` lookup, a `DELETE` refused and recorded
- [ ] 5.2 Write-back: read, flag, archive, delete and restore, spam, streams by folder and by category, no echo, never a `DELETE`
- [ ] 5.3 Conflicts: a 412 re-read then made; Graph's later change wins
- [ ] 5.4 A move keeps the index; a move answering another id
- [ ] 5.5 Throttling of a push and of a send (`Retry-After`)
- [ ] 5.6 Sending: once, no Sent echo, the next sync deduped, threading headers, over the limit, unverified address, another From, an unclear attempt
- [ ] 5.7 Microsoft mail off: no write-back, no send; production's settings still start

## 6. Finish
- [ ] 6.1 `deploy/README.md` (write-back, sending, `xms_edov` for sending)
- [ ] 6.2 OpenAPI unchanged (no operation); client types unchanged
- [ ] 6.3 Tracking plan unchanged (no event, no Coverage row)
- [ ] 6.4 Changelog entry `changelog/unreleased/added/317-microsoft-mail-write.md`
- [ ] 6.5 Gates green in CI; no UI change (backend recording)
