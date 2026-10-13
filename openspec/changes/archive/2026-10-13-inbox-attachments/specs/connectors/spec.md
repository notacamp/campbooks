## ADDED Requirements

### Requirement: Inline parts keep their Content-ID
When the Gmail adapter stores a message's attachments, each one SHALL carry
the part's `Content-ID` without its angle brackets (as `content_id` in the
MessageChangeBatch v1 attachment, optional, at most 255 characters; a
longer one is left out), so that the body's `cid:` images can be resolved.

#### Scenario: An inline logo
- **WHEN** a message's HTML part shows `cid:logo@example.com` and a `multipart/related` part has `Content-ID: <logo@example.com>`
- **THEN** the upsert's attachment for that part has `content_id` `logo@example.com` and `inline` true
