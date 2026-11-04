## 1. Fix

- [x] 1.1 Zoho attachment upload sends `Content-Type: application/octet-stream`

## 2. Prove it

- [x] 2.1 Fake Zoho refuses non-octet uploads with 415 and records uploads
- [x] 2.2 Flow test: a Zoho reply with a PDF and a JPEG is sent, both uploaded as they are
