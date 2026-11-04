## Why

Every message with an attachment sent from a Zoho inbox failed with "Your mail
provider refused this message. Check the addresses and try again." The
addresses were fine. Before sending, Campbooks uploads each attachment to Zoho
(`POST messages/attachments?fileName=…`) and sent the file's own media type as
the request's `Content-Type` (`application/pdf`, `image/jpeg`, …). Zoho takes
the upload only as a raw stream of octets, so it answered `415 Unsupported
Media Type`, and the send ended `rejected`. Prod logs on 2026-11-04 show two
such sends, both `HTTP 415`.

The fake Zoho in the tests accepted any upload, and no test sent a Zoho
message with attachments, so this was never caught.

## What Changes

- An attachment upload to Zoho is sent as `application/octet-stream`,
  whatever the file is. Zoho names the file by the `fileName` it gets.
- The fake Zoho refuses an upload that isn't `application/octet-stream`
  with 415, as Zoho does, and records what was uploaded.
- A new flow test sends a Zoho reply with a PDF and a JPEG. Both go up as
  they are, and the one send names both.

Server only; no API, client or flag change.
