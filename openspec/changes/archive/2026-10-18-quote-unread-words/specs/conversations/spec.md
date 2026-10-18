## ADDED Requirements

### Requirement: A message's words alone
`conversations.get_message_body` with `words=true` SHALL return the
message's words alone, as `text`, with no `html`: its text part when it has
words, else its HTML read as text the way every reader reads it (at most
500 ms, at most 256 KiB of the HTML), cut at 256 KiB and then marked
`truncated`. It SHALL need exactly the access of the body. This is what a
reply or forward quotes when `conversations.get` marked the message
`text_unread`. In a debug build only, `CAMPBOOKS_DEV_TEXT_DEADLINE_MS`, when
set, SHALL replace the one-second deadline `conversations.get` reads HTML
within (0 marks every message read from HTML `text_unread`); release builds
SHALL never read it.

#### Scenario: Words of HTML-only mail
- **WHEN** a member asks for the words of a message that is HTML alone
- **THEN** `text` holds the HTML's words and `html` is absent

#### Scenario: Words of a message with a text part
- **WHEN** a member asks for the words of a message with a text part
- **THEN** `text` is that text part

#### Scenario: Someone outside the inbox
- **WHEN** a member of the workspace who is not in the message's inbox asks for its words
- **THEN** the message is not found

#### Scenario: Forcing text_unread for a recording
- **WHEN** a debug server runs with `CAMPBOOKS_DEV_TEXT_DEADLINE_MS=0` and a member opens a conversation of HTML-only mail
- **THEN** each such message comes without `text`, marked `text_unread`
