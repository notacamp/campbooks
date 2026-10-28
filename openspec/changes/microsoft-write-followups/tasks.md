# Tasks

## 1. Sending
- [ ] 1.1 `send_message/via_microsoft.rs`: a 202 always ends sent, with no provider id when the copy is not found
- [ ] 1.2 A 429 on `sendMail` marks the connection `throttled` and holds the stream until `Retry-After`

## 2. Write-back
- [ ] 2.1 `microsoft/write.rs`: only a 404 at the read is gone; other failures end `failed` (`refused`) and re-emit
- [ ] 2.2 `microsoft/write.rs`: re-emit a message whose move had nowhere to go
- [ ] 2.3 Restore folder recorded before the move; `outbound::verified` accepts a pending change bound with its place
- [ ] 2.4 `outbound::unsupported` and `sync_stream`: pending Microsoft changes end `unsupported` when Microsoft mail is off

## 3. Tests
- [ ] 3.1 Fake Graph: refused read, move whose answer is lost, rewritten `Message-ID`
- [ ] 3.2 One test per point 1-5 and 7; nothing deleted checked after every write and send

## 4. Finish
- [ ] 4.1 OpenAPI unchanged (no operation); client types unchanged
- [ ] 4.2 Tracking plan unchanged (no event, no Coverage row)
- [ ] 4.3 Changelog entry file
- [ ] 4.4 Gates green in CI; no UI change (backend recording)
