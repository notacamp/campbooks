## 1. Server

- [x] 1.1 Connectors: keep each part's `Content-ID` (parser, contract, MessageChangeBatch v1 schema)
- [x] 1.2 Conversations: `attachments.content_id` migration; ingest and apply carry it
- [x] 1.3 `conversations.get_attachment`: authz, safe types, disposition, file names, headers; tests (other inbox 404, headers, malicious name, HTML never inline, safe list)
- [x] 1.4 Sanitiser: placeholder for blocked images, `cid:` images as `data:` URLs; `get_message_body` resolves inline parts within limits; tests
- [x] 1.5 Web CSP: `blob:` in `img-src`

## 2. Client

- [x] 2.1 `lib/ui`: `AttachmentList`, `ImagePreview`, download icon, `MessageCard` files slot; stories; tests
- [x] 2.2 Conversations: download / preview through the API client, V shortcut, i18n in four languages
- [x] 2.3 Fake Google: a message with a photo, an inline logo, a remote image and an HTML attachment

## 3. Wrap-up

- [x] 3.1 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated
- [x] 3.2 CHANGELOG updated
- [x] 3.3 Gates green
- [x] 3.4 UI verified and recorded at 375px and desktop
