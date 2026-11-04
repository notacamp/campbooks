## 1. Server

- [x] 1.1 `refused_as`: a definite refusal fails with the given reason; Zoho's attachment upload passes `attachment_refused`
- [x] 1.2 Contract `connectors.send_completed.v1` and the event/draft docs name the reason; `openapi/events.json` regenerated
- [x] 1.3 Tracking maps `attachment_refused` to `rejected`

## 2. Client

- [x] 2.1 `attachment_refused` is a known error with copy in en, pt-PT, es-ES and fr

## 3. Prove it

- [x] 3.1 Fake Zoho can refuse uploads; flow test: the send fails `attachment_refused` and nothing is sent
- [x] 3.2 ReplyComposer test: the failed draft says the attachment was refused, not the addresses
- [x] 3.3 Recording at 375 and desktop
